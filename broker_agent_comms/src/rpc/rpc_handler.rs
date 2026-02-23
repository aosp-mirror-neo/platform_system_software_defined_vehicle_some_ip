// Copyright (C) 2026 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use async_trait::async_trait;
use dashmap::DashMap;
use mockall::automock;
use sdv_comms::sd::ServiceUnitDefinition;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::Server;
use sdv_comms::rpc::UnaryRpcMethod;
use sdv_comms::sd::ServerUnitMetadata;
use sdv_comms::sd::UnitMetadata;
use sdv_comms::sd::UnitRegistrationArgs;
use sdv_comms::sd::UnitType;
use sdv_status_rs::SdvStatus;
use sdv_status_rs::SdvStatusCode;

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;

use crate::agent::protocol_mapping::RpcMethodMappingInfo;
use crate::agent::protocol_mapping::SdvIdentifier;
use crate::agent::protocol_mapping::SomeIpRpcMappingInfo;
use crate::agent::protocol_mapping::SomeIpServiceInstanceInfo;
use crate::agent::service_matcher::SomeIpServiceMap;
use crate::agent::IAvailabilitySubscriber;
use crate::agent::ServiceType;
use crate::comms::ISdvCommsContext;
use crate::comms::ISomeIpStackContext;
use crate::rpc::config::{BrokerRpcClientInfo, BrokerSomeIpService, SdvRpcInterfaceInfo};
use crate::rpc::RpcRequestCallback;
use crate::translator::IBrokerTranslator;
use crate::translator::TranslationMode;
use crate::translator::{RpcMode, SomeIpDescriptorId};

type ConnectionTaskMap = Arc<DashMap<BrokerSomeIpService, (CancellationToken, JoinHandle<()>)>>;

#[automock]
#[async_trait]
pub(crate) trait IRpcHandler: Send + Sync {
    fn add_sdv_rpc_mapping(
        &mut self,
        service_unit_info: SdvIdentifier,
        someip_service_instance_info: SomeIpService,
        rpc_mapping_info: Vec<RpcMethodMappingInfo>,
    );
    fn add_someip_rpc_mapping(
        &mut self,
        someip_service_instance_info: SomeIpServiceInstanceInfo,
        rpc_mapping_info: Vec<SomeIpRpcMappingInfo>,
    );
    fn create_rpc_request_callback(&self) -> RpcRequestCallback;
    async fn start(
        &self,
        mut availability_subscriber: Box<dyn IAvailabilitySubscriber>,
        someip_comms: Arc<dyn ISomeIpStackContext>,
    ) -> anyhow::Result<()>;
}

pub(crate) struct RpcHandler {
    sdv_comms: Arc<dyn ISdvCommsContext>,
    translator: Arc<dyn IBrokerTranslator>,
    broker_rpc_client_map: Arc<DashMap<BrokerSomeIpService, BrokerRpcClientInfo>>,
    sdv_service_mappings: HashMap<SdvIdentifier, SdvRpcInterfaceInfo>,
    someip_service_mappings: SomeIpServiceMap<Vec<SomeIpRpcMappingInfo>>,
    connection_tasks: ConnectionTaskMap,
}

impl RpcHandler {
    pub(crate) fn new(
        sdv_comms: Arc<dyn ISdvCommsContext>,
        translator: Arc<dyn IBrokerTranslator>,
    ) -> Self {
        Self {
            sdv_comms,
            translator,
            broker_rpc_client_map: Arc::new(DashMap::new()),
            sdv_service_mappings: HashMap::new(),
            someip_service_mappings: SomeIpServiceMap::new(),
            connection_tasks: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl IRpcHandler for RpcHandler {
    fn add_sdv_rpc_mapping(
        &mut self,
        sdv_identifier: SdvIdentifier,
        someip_service_instance_info: SomeIpService,
        rpc_mapping_info: Vec<RpcMethodMappingInfo>,
    ) {
        self.sdv_service_mappings.insert(
            sdv_identifier,
            SdvRpcInterfaceInfo::new(someip_service_instance_info, rpc_mapping_info),
        );
    }

    fn add_someip_rpc_mapping(
        &mut self,
        someip_service_instance_info: SomeIpServiceInstanceInfo,
        rpc_mapping_info: Vec<SomeIpRpcMappingInfo>,
    ) {
        self.someip_service_mappings.insert(
            someip_service_instance_info.someip_service_instance,
            someip_service_instance_info.sd_minor_version_mode,
            rpc_mapping_info,
        );
    }

    fn create_rpc_request_callback(&self) -> RpcRequestCallback {
        RpcRequestCallback::create(self.broker_rpc_client_map.clone(), self.translator.clone())
    }

    async fn start(
        &self,
        mut availability_subscriber: Box<dyn IAvailabilitySubscriber>,
        someip_comms: Arc<dyn ISomeIpStackContext>,
    ) -> anyhow::Result<()> {
        let broker_rpc_server_map = Arc::new(DashMap::new());

        while let Some(availability_message) = availability_subscriber.receive().await {
            match availability_message.service {
                ServiceType::SDV(service_unit_definition) => {
                    self.handle_sdv_unit_availability(
                        &someip_comms,
                        service_unit_definition,
                        availability_message.available,
                    )
                    .await;
                }
                ServiceType::SOMEIP(someip_service) => {
                    let Some(someip_rpc_mapping_infos) =
                        self.someip_service_mappings.get(&someip_service)
                    else {
                        // service has no RPC Interfaces to handle
                        continue;
                    };
                    let someip_comms_clone = someip_comms.clone();
                    let broker_rpc_server_map_clone = broker_rpc_server_map.clone();
                    let someip_rpc_mapping_infos_clone = someip_rpc_mapping_infos.clone();
                    let sdv_comms_clone = self.sdv_comms.clone();
                    let translator_clone = self.translator.clone();
                    tokio::spawn(async move {
                        Self::handle_someip_service_availability(
                            someip_comms_clone,
                            someip_service,
                            availability_message.available,
                            broker_rpc_server_map_clone,
                            someip_rpc_mapping_infos_clone,
                            sdv_comms_clone,
                            translator_clone,
                        )
                        .await;
                    });
                }
            }
        }

        return Err(anyhow::Error::msg("Availability subscriber channel has been closed!"));
    }
}

impl RpcHandler {
    fn find_matching_info(
        &self,
        service_unit_definition: &ServiceUnitDefinition,
    ) -> Option<&SdvRpcInterfaceInfo> {
        for (sdv_identifier, sdv_rpc_interface_info) in &self.sdv_service_mappings {
            match sdv_identifier {
                SdvIdentifier::ServiceUnitInfo { service_unit_id } => {
                    if service_unit_id.service_unit_name
                        == service_unit_definition.get_unit_identifier().get_unit_name()
                        && service_unit_id.service_fqin.matches(
                            service_unit_definition.get_service_bundle_identity().get_fqin(),
                        )
                    {
                        return Some(sdv_rpc_interface_info);
                    }
                }
                SdvIdentifier::SoaInfo { soa_identifier: _ } => {
                    // TODO: b/487204223 - implement SOA logic in SOME/IP broker
                }
            }
        }
        None
    }

    async fn handle_sdv_unit_availability(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        service_unit_definition: Box<ServiceUnitDefinition>,
        available: bool,
    ) {
        let Some(sdv_rpc_interface_info) = self.find_matching_info(&service_unit_definition) else {
            // service has no RPC interface to handle
            return;
        };
        let someip_service = BrokerSomeIpService::from_someip_service(
            sdv_rpc_interface_info.someip_service_instance_info,
        );
        if available {
            self.spawn_connection_task(
                someip_comms.clone(),
                service_unit_definition,
                sdv_rpc_interface_info.clone(),
            );
        } else {
            if let Some((_, (token, handle))) = self.connection_tasks.remove(&someip_service) {
                token.cancel();
                // Errors are ignored as we can't do much if task is already finished
                let _ = handle.await;
            }
            if let Err(e) = someip_comms
                .stop_offer_service(&sdv_rpc_interface_info.someip_service_instance_info)
                .await
            {
                log::error!("{e:?}");
            }
            let _ = self.broker_rpc_client_map.remove(&someip_service);
        }
    }

    fn spawn_connection_task(
        &self,
        someip_comms: Arc<dyn ISomeIpStackContext>,
        service_unit_definition: Box<ServiceUnitDefinition>,
        sdv_rpc_interface_info: SdvRpcInterfaceInfo,
    ) {
        let token = CancellationToken::new();
        let token_clone = token.clone();
        let sdv_comms = self.sdv_comms.clone();
        let broker_rpc_client_map = self.broker_rpc_client_map.clone();

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased; // Always process cancellation first
                    _ = token_clone.cancelled() => {
                        log::info!("Connection task cancelled for service: {service_unit_definition:?}");
                        break;
                    }
                    result = sdv_comms.create_client(service_unit_definition.get_unit_identifier()) => {
                        let rpc_client = match result {
                            Ok(client) => client,
                            Err(e) => {
                                // SD notification should only mark service as available when we ready to connect.
                                // In exceptional cases retry may still be needed, but we keep logic for it simple and duration short.
                                const RETRY_DELAY: Duration = Duration::from_millis(100);
                                log::warn!(
                                    "Unable to create client for SDV service {:?}, Error {e}. Retrying in {RETRY_DELAY:?}...",
                                    service_unit_definition.get_unit_identifier()
                                );

                                tokio::time::sleep(RETRY_DELAY).await;
                                continue;
                            }
                        };

                        let interface_methods: HashMap<_, _> = sdv_rpc_interface_info
                            .rpc_mapping_info
                            .iter()
                            .map(|info| (info.someip_method_id, info.sdv_method_name.clone()))
                            .collect();

                        // In rare cases we might have old value from the previous attempt. Override it with new value.
                        let _ignore_old_value = broker_rpc_client_map.insert(
                            BrokerSomeIpService::from_someip_service(
                                sdv_rpc_interface_info.someip_service_instance_info,
                            ),
                            BrokerRpcClientInfo::new(
                                rpc_client,
                                service_unit_definition.clone(),
                                interface_methods,
                            ),
                        );

                        if let Err(e) = someip_comms
                            .offer_service(&sdv_rpc_interface_info.someip_service_instance_info)
                            .await
                        {
                            // We don't expect this to happen often on properly configured system. We are not aware of practical
                            // situations where a specific service can recover from this error. However retry with significant
                            //  delay to attempt recovery.
                            // Client in broker_rpc_client_map will be cleared later.
                            const RETRY_DELAY: Duration = Duration::from_secs(1);
                            log::warn!(
                                "Unable to offer SOME/IP service {:?}, Error {e:?}. Retrying in {RETRY_DELAY:?}...",
                                 service_unit_definition.get_unit_identifier());

                            tokio::time::sleep(RETRY_DELAY).await;
                            continue;
                        }


                        // Connection successful, exit the task.
                        break;
                    }
                }
            }
        });
        self.connection_tasks.insert(
            BrokerSomeIpService::from_someip_service(
                sdv_rpc_interface_info.someip_service_instance_info,
            ),
            (token, task),
        );
    }

    async fn handle_someip_service_availability(
        someip_comms: Arc<dyn ISomeIpStackContext>,
        someip_service: SomeIpService,
        available: bool,
        broker_rpc_server_map: Arc<DashMap<SomeIpService, Server>>,
        someip_rpc_mapping_infos: Vec<SomeIpRpcMappingInfo>,
        sdv_comms: Arc<dyn ISdvCommsContext>,
        translator: Arc<dyn IBrokerTranslator>,
    ) {
        if available {
            for someip_rpc_mapping_info in &someip_rpc_mapping_infos {
                Self::handle_someip_rpc_mapping_info(
                    &someip_comms,
                    &someip_service,
                    someip_rpc_mapping_info,
                    &broker_rpc_server_map,
                    &sdv_comms,
                    &translator,
                )
                .await;
            }
        } else {
            // Server Drop impl should be called here
            let _ = broker_rpc_server_map.remove(&someip_service);
        }
    }

    async fn handle_someip_rpc_mapping_info(
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        someip_service: &SomeIpService,
        someip_rpc_mapping_info: &SomeIpRpcMappingInfo,
        broker_rpc_server_map: &Arc<DashMap<SomeIpService, Server>>,
        sdv_comms: &Arc<dyn ISdvCommsContext>,
        translator: &Arc<dyn IBrokerTranslator>,
    ) {
        let mut method_map = HashMap::new();
        let broker_fqin = sdv_comms.get_fqin().await;
        for rpc_method_info in &someip_rpc_mapping_info.rpc_method_infos {
            let unary_rpc_method = Self::create_unary_rpc_method(
                Arc::clone(someip_comms),
                someip_rpc_mapping_info.broker_service_unit.unit_type.clone(),
                *someip_service,
                rpc_method_info.clone(),
                Arc::clone(translator),
                broker_fqin.clone(),
            );
            method_map.insert(rpc_method_info.sdv_method_name.clone(), unary_rpc_method);
        }

        let mut unit_registration_args = UnitRegistrationArgs::new();
        {
            let mut server_unit_metadata = ServerUnitMetadata::new();
            server_unit_metadata.set_version(1);
            unit_registration_args
                .set_unit_name(&someip_rpc_mapping_info.broker_service_unit.service_unit_name);
            unit_registration_args
                .set_unit_type(&someip_rpc_mapping_info.broker_service_unit.unit_type);
            unit_registration_args.set_unit_metadata(&UnitMetadata::Server(server_unit_metadata));
        }
        let Ok(server) = sdv_comms.create_server(unit_registration_args, method_map).await else {
            log::error!("Error while creating RPC server");
            return;
        };
        broker_rpc_server_map.insert(*someip_service, server);
    }

    fn create_unary_rpc_method(
        someip_comms: Arc<dyn ISomeIpStackContext>,
        unit_type: UnitType,
        someip_service: SomeIpService,
        method_mapping_info: RpcMethodMappingInfo,
        translator: Arc<dyn IBrokerTranslator>,
        broker_fqin: ServiceFqin,
    ) -> UnaryRpcMethod {
        Arc::new(move |_, method_data: &[u8]| {
            let method_translator = translator.clone();
            let someip_comms = someip_comms.clone();
            let unit_type = unit_type.clone();
            let method_mapping_info = method_mapping_info.clone();
            let data = method_data.to_vec();
            let broker_fqin = broker_fqin.clone();
            Box::pin(async move {
                let someip_bytes = method_translator
                    .translate_from_protobuf_to_someip(
                        &broker_fqin,
                        &unit_type,
                        &data,
                        TranslationMode::RpcRequest(method_mapping_info.sdv_method_name.clone()),
                    )
                    .await
                    .map_err(|e| {
                        SdvStatus::with_message(SdvStatusCode::Internal, format!("{e:?}"))
                    })?;

                let response_someip_bytes = someip_comms
                    .rpc_transact(
                        &someip_service,
                        method_mapping_info.someip_method_id,
                        someip_bytes,
                    )
                    .await
                    .map_err(|e| {
                        SdvStatus::with_message(SdvStatusCode::Internal, format!("{e:?}"))
                    })?;

                let descriptor_id = SomeIpDescriptorId::new_method_id(
                    someip_service.service_id,
                    someip_service.version.major as u8,
                    method_mapping_info.someip_method_id,
                    RpcMode::Response,
                );

                Ok(method_translator
                    .translate_from_someip_to_protobuf_rpc(
                        &unit_type,
                        &response_someip_bytes,
                        &descriptor_id,
                    )
                    .await
                    .map_err(|e| {
                        SdvStatus::with_message(
                            SdvStatusCode::Internal,
                            format!("Error translating SOME/IP to protobuf response. {e:?}"),
                        )
                    })?
                    .get_vec())
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::notifier::MockIAvailabilitySubscriber;
    use crate::agent::protocol_mapping::{
        BrokerServiceUnit, RpcMethodMappingInfo, SomeIpRpcMappingInfo,
    };
    use crate::comms::{MockISdvCommsContext, MockISomeIpStackContext};
    use crate::translator::MockIBrokerTranslator;
    use google_sdv_someip::aidl::google::sdv::someip::{
        SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
    };
    use sdv_comms::id::ServiceFqin;
    use sdv_comms::sd::{UnitRegistrationArgs, UnitType};
    use someip_config::broker_params::SdMinorVersionMode;
    use std::collections::HashMap;

    fn create_test_someip_service(
        service_id: u16,
        instance_id: u16,
        major: u8,
        minor: u32,
    ) -> SomeIpService {
        SomeIpService {
            service_id,
            instance_id,
            version: SomeIpServiceVersion { major: major as i8, minor: minor as i32 },
        }
    }

    #[tokio::test]
    async fn start_handles_someip_available_rpc() {
        // GIVEN
        let mut sdv_comms = MockISdvCommsContext::new();
        let translator = MockIBrokerTranslator::new();
        let someip_comms = MockISomeIpStackContext::new();
        let mut availability_subscriber = MockIAvailabilitySubscriber::new();

        let someip_service = create_test_someip_service(1, 1, 1, 0);
        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.test")
            .set_service_bundle_name("Broker")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unit type must be valid");
        let service_unit_name = "test-unit";
        let broker_service_unit =
            BrokerServiceUnit { service_unit_name: service_unit_name.to_string(), unit_type };
        let rpc_mapping_info = vec![SomeIpRpcMappingInfo {
            broker_service_unit: broker_service_unit.clone(),
            rpc_method_infos: vec![RpcMethodMappingInfo {
                sdv_method_name: "test_method".to_string(),
                someip_method_id: 100,
            }],
        }];

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.sdv.someip")
            .service_bundle_name("SomeIpBroker")
            .service_instance_name("default")
            .build()
            .expect("Broker FQIN must be valid");

        sdv_comms.expect_get_fqin().return_const(broker_fqin.clone());

        sdv_comms
            .expect_create_server()
            .withf(move |args: &UnitRegistrationArgs, _: &HashMap<String, UnaryRpcMethod>| {
                args.get_unit_name() == service_unit_name
                    && args.get_unit_type() == broker_service_unit.unit_type
            })
            .times(1)
            // TODO(b/433173907): Servers can't be created from test.
            .returning(|_, _| Err(anyhow::Error::msg("Cannot create a server in tests")));

        let availability_message = crate::agent::AvailabilityMessage {
            service: ServiceType::SOMEIP(someip_service),
            available: true,
        };

        availability_subscriber
            .expect_receive()
            .times(1)
            .return_once(move || Some(availability_message));
        availability_subscriber.expect_receive().times(1).return_once(|| None);

        // WHEN
        let mut handler = RpcHandler::new(Arc::new(sdv_comms), Arc::new(translator));
        let someip_service_instance = SomeIpServiceInstanceInfo::new(
            someip_service,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        handler.add_someip_rpc_mapping(someip_service_instance, rpc_mapping_info.clone());

        // THEN
        let handler_arc = Arc::new(handler);
        let timeout = Duration::from_secs(1);
        let task = tokio::spawn(tokio::time::timeout(timeout, async move {
            let result =
                handler_arc.start(Box::new(availability_subscriber), Arc::new(someip_comms)).await;
            assert!(result.is_err());
        }));
        if let Err(e) = task.await {
            log::error!("Error while awaiting task {e:?}");
        }
    }

    #[tokio::test]
    async fn start_handles_someip_unavailable_rpc() {
        // GIVEN
        let sdv_comms = MockISdvCommsContext::new();
        let translator = MockIBrokerTranslator::new();
        let someip_comms = MockISomeIpStackContext::new();
        let mut availability_subscriber = MockIAvailabilitySubscriber::new();

        let someip_service = create_test_someip_service(1, 1, 1, 0);
        let rpc_mapping_info = vec![SomeIpRpcMappingInfo {
            broker_service_unit: BrokerServiceUnit {
                service_unit_name: "test-unit".to_string(),
                unit_type: UnitType::builder()
                    .set_sdv_package_name("com.sdv.test")
                    .set_service_bundle_name("Broker")
                    .set_unit_type_name("TestType")
                    .build()
                    .expect("Unit type must be valid"),
            },
            rpc_method_infos: vec![RpcMethodMappingInfo {
                sdv_method_name: "test_method".to_string(),
                someip_method_id: 100,
            }],
        }];

        let availability_message = crate::agent::AvailabilityMessage {
            service: ServiceType::SOMEIP(someip_service),
            available: false,
        };

        availability_subscriber
            .expect_receive()
            .times(1)
            .return_once(move || Some(availability_message));
        availability_subscriber.expect_receive().times(1).return_once(|| None);

        // WHEN
        let mut handler = RpcHandler::new(Arc::new(sdv_comms), Arc::new(translator));
        let someip_service_instance = SomeIpServiceInstanceInfo::new(
            someip_service,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        handler.add_someip_rpc_mapping(someip_service_instance, rpc_mapping_info.clone());

        // THEN
        let handler_arc = Arc::new(handler);
        let timeout = Duration::from_secs(1);
        let task = tokio::spawn(tokio::time::timeout(timeout, async move {
            let result =
                handler_arc.start(Box::new(availability_subscriber), Arc::new(someip_comms)).await;
            assert!(result.is_err());
        }));
        if let Err(e) = task.await {
            log::error!("Error while awaiting task {e:?}");
        }
    }
}
