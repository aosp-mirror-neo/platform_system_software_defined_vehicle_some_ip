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

use std::sync::Arc;

use tokio::join;

use crate::agent::protocol_mapping::ProtocolMapping;
use crate::agent::protocol_mapping::SdvServiceUnitInterface;

use crate::agent::ChannelMessageFactory;
use crate::agent::IAvailabilityMessageFactory;
use crate::binder_load_indicators::load_indicators::LoadIndicators;
use crate::comms::ISomeIpStackContext;
use crate::comms::ISomeIpStackContextFactory;
use crate::pubsub::IPubSubHandler;
use crate::rpc::IRpcHandler;
use crate::service_discovery::IServiceDiscoveryHandler;

pub(crate) struct SomeIpBroker {
    protocol_mapping: ProtocolMapping,
    someip_context_factory: Box<dyn ISomeIpStackContextFactory>,
    sd_handler: Arc<dyn IServiceDiscoveryHandler>,
    pubsub_handler: Arc<dyn IPubSubHandler>,
    rpc_handler: Arc<dyn IRpcHandler>,
    availability_message_factory: Box<dyn IAvailabilityMessageFactory>,
}

impl SomeIpBroker {
    pub(crate) fn new(
        protocol_mapping: ProtocolMapping,
        someip_context_factory: Box<dyn ISomeIpStackContextFactory>,
        sd_handler: Box<dyn IServiceDiscoveryHandler>,
        pubsub_handler: Box<dyn IPubSubHandler>,
        rpc_handler: Box<dyn IRpcHandler>,
        availability_message_queue: usize,
    ) -> Self {
        SomeIpBroker {
            protocol_mapping,
            someip_context_factory,
            sd_handler: Arc::from(sd_handler),
            pubsub_handler: Arc::from(pubsub_handler),
            rpc_handler: Arc::from(rpc_handler),
            availability_message_factory: Box::new(ChannelMessageFactory::new(
                availability_message_queue,
            )),
        }
    }

    pub(crate) async fn start(
        &mut self,
        load_indicators: Option<Arc<LoadIndicators>>,
    ) -> anyhow::Result<()> {
        self.fill_broker_mappings()?;

        let (pubsub_message_notifier, pubsub_message_subscriber) =
            self.availability_message_factory.create_messaging_channel()?;

        let (rpc_message_notifier, rpc_message_subscriber) =
            self.availability_message_factory.create_messaging_channel()?;

        let availability_callback = self.sd_handler.create_availability_callback(vec![
            pubsub_message_notifier.clone_box(),
            rpc_message_notifier.clone_box(),
        ]);
        let event_notification_callback =
            self.pubsub_handler.create_event_notification_callback(load_indicators);
        let rpc_request_callback = self.rpc_handler.create_rpc_request_callback();

        let someip_comms = self
            .someip_context_factory
            .create(availability_callback, event_notification_callback, rpc_request_callback)
            .await?;
        let someip_comms: Arc<dyn ISomeIpStackContext> = Arc::from(someip_comms);

        let task_pubsub_handler = self.pubsub_handler.clone();
        let task_pubsub_someip_comms = someip_comms.clone();
        let pubsub_join_handle = tokio::spawn(async move {
            let task_return = task_pubsub_handler
                .start(pubsub_message_subscriber, task_pubsub_someip_comms)
                .await;
            log::info!("Pub/Sub Handler returned with value {task_return:?}");
        });

        let task_rpc_handler = self.rpc_handler.clone();
        let task_rpc_someip_comms = someip_comms.clone();
        let rpc_join_handle = tokio::spawn(async move {
            let task_return =
                task_rpc_handler.start(rpc_message_subscriber, task_rpc_someip_comms).await;
            log::info!("RPC Handler returned with value {task_return:?}");
        });

        let task_sd_handler = self.sd_handler.clone();
        let sd_join_handle = tokio::spawn(async move {
            task_sd_handler
                .start(vec![pubsub_message_notifier, rpc_message_notifier], someip_comms)
                .await;
        });

        log::debug!("SOME/IP Broker has started successfully");
        tracing::event!(tracing::Level::DEBUG, "SOME/IP Broker has started successfully");
        let (_, _, sd_ret) = join!(pubsub_join_handle, rpc_join_handle, sd_join_handle);

        log::info!("SOME/IP Broker terminating!");
        log::info!("Service Discovery Handler returned {sd_ret:?}");

        Ok(())
    }

    fn fill_broker_mappings(&mut self) -> anyhow::Result<()> {
        let sd_handler = Arc::get_mut(&mut self.sd_handler).ok_or(anyhow::Error::msg(
            "Unable to get a mutable refererence to the ServiceDiscovery handler",
        ))?;
        let pubsub_handler = Arc::get_mut(&mut self.pubsub_handler).ok_or(anyhow::Error::msg(
            "Unable to get a mutable refererence to the Pub/Sub handler",
        ))?;
        let rpc_handler = Arc::get_mut(&mut self.rpc_handler)
            .ok_or(anyhow::Error::msg("Unable to get a mutable refererence to the RPC handler"))?;

        for sdv_service_unit in &self.protocol_mapping.sdv_service_units {
            sd_handler.add_sdv_handle(
                sdv_service_unit.sdv_identifier.clone(),
                sdv_service_unit.unit_type.clone(),
            );

            match &sdv_service_unit.service_unit_interface {
                SdvServiceUnitInterface::TOPIC(topic_mapping_info) => {
                    pubsub_handler.add_topic_mapping(
                        sdv_service_unit.sdv_identifier.clone(),
                        sdv_service_unit.someip_service_instance_info,
                        topic_mapping_info.clone(),
                    );
                }
                SdvServiceUnitInterface::RPC(rpc_mapping_info) => {
                    rpc_handler.add_sdv_rpc_mapping(
                        sdv_service_unit.sdv_identifier.clone(),
                        sdv_service_unit.someip_service_instance_info,
                        rpc_mapping_info.clone(),
                    );
                }
            }
        }

        for sdv_multipub_topic in &self.protocol_mapping.sdv_multipub_topics {
            sd_handler.add_sdv_multipub_handle(sdv_multipub_topic.unit_type.clone());
            pubsub_handler.add_multpub_topic_mapping(
                sdv_multipub_topic.unit_type.clone(),
                sdv_multipub_topic.someip_service_instance_info,
                sdv_multipub_topic.someip_eventgroup_id,
                sdv_multipub_topic.someip_event_id,
            );
        }

        for someip_service_instance in &self.protocol_mapping.someip_service_instances {
            sd_handler.add_someip_handle(someip_service_instance.someip_service_instance_info);
            if !someip_service_instance.event_infos.is_empty() {
                pubsub_handler.add_event_mapping(
                    someip_service_instance.someip_service_instance_info,
                    someip_service_instance.event_infos.clone(),
                );
            }
            if !someip_service_instance.rpc_infos.is_empty() {
                rpc_handler.add_someip_rpc_mapping(
                    someip_service_instance.someip_service_instance_info,
                    someip_service_instance.rpc_infos.clone(),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use dashmap::DashMap;
    use mockall::predicate::eq;
    use someip_config::broker_params::SdMinorVersionMode;
    use std::sync::Arc;

    use sdv_comms::sd::UnitType;

    use google_sdv_someip::aidl::google::sdv::someip::{
        SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
    };

    use crate::agent::protocol_mapping::SomeIpServiceInstanceInfo;
    use crate::agent::protocol_mapping::*;
    use crate::agent::SomeIpBroker;
    use crate::pubsub::EventNotificationCallback;
    use crate::rpc::RpcRequestCallback;
    use crate::service_discovery::AvailabilityCallback;

    use crate::agent::notifier::{
        MockIAvailabilityMessageFactory, MockIAvailabilityNotifier, MockIAvailabilitySubscriber,
    };
    use crate::comms::{MockISomeIpStackContext, MockISomeIpStackContextFactory};
    use crate::pubsub::MockIPubSubHandler;
    use crate::rpc::MockIRpcHandler;
    use crate::service_discovery::MockIServiceDiscoveryHandler;
    use crate::translator::MockIBrokerTranslator;

    fn create_default_sdv_service_mapping(
        service_unit_interface: SdvServiceUnitInterface,
    ) -> SdvServiceUnitMapping {
        SdvServiceUnitMapping {
            sdv_identifier: SdvIdentifier::ServiceUnitInfo {
                service_unit_id: SdvServiceUnitId {
                    service_fqin: ServiceFqinInternal {
                        sdv_vm_name: String::from("test_vm_name").into(),
                        sdv_package_name: String::from("com.test.package"),
                        service_bundle_name: String::from("TestBundleName"),
                        service_instance_name: String::from("test_instance_name").into(),
                    },
                    service_unit_name: "test_service_unit".to_string(),
                },
            },
            unit_type: UnitType::builder()
                .set_sdv_package_name("com.test.package")
                .set_service_bundle_name("TestBundle")
                .set_unit_type_name("TestUnitType")
                .build()
                .unwrap(),
            someip_service_instance_info: SomeIpService {
                service_id: 2,
                instance_id: 4,
                version: SomeIpServiceVersion { major: 2, minor: 1 },
            },
            service_unit_interface,
        }
    }

    fn create_default_someip_service_mapping(
        event_infos: Vec<SomeIpEventMappingInfo>,
        rpc_infos: Vec<SomeIpRpcMappingInfo>,
    ) -> SomeIpServiceInstanceMapping {
        SomeIpServiceInstanceMapping {
            someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                SomeIpService {
                    service_id: 2,
                    instance_id: 4,
                    version: SomeIpServiceVersion { major: 2, minor: 1 },
                },
                SdMinorVersionMode::ExactOrAnyMinorVersion,
            ),
            event_infos,
            rpc_infos,
        }
    }

    fn create_someip_event_mapping(
        event_id: u16,
        eventgroup_id: u16,
        event_mapping_type: SomeIpEventMappingType,
    ) -> SomeIpEventMappingInfo {
        SomeIpEventMappingInfo { event_id, eventgroup_id, event_mapping_type }
    }

    fn set_default_expectations(
        someip_context_factory: &mut MockISomeIpStackContextFactory,
        availability_message_factory: &mut MockIAvailabilityMessageFactory,
        pubsub_handler: &mut MockIPubSubHandler,
        rpc_handler: &mut MockIRpcHandler,
        sd_handler: &mut MockIServiceDiscoveryHandler,
    ) {
        availability_message_factory.expect_create_messaging_channel().times(2).returning(|| {
            let mut notifier_mock = MockIAvailabilityNotifier::new();
            notifier_mock
                .expect_clone_box()
                .times(1)
                .returning(|| Box::new(MockIAvailabilityNotifier::new()));
            Ok((Box::new(notifier_mock), Box::new(MockIAvailabilitySubscriber::new())))
        });
        pubsub_handler.expect_create_event_notification_callback().times(1).returning(move |_| {
            EventNotificationCallback::create(
                Arc::new(DashMap::new()),
                Arc::new(MockIBrokerTranslator::new()),
                None,
            )
        });
        rpc_handler.expect_create_rpc_request_callback().times(1).returning(|| {
            RpcRequestCallback::create(
                Arc::new(DashMap::new()),
                Arc::new(MockIBrokerTranslator::new()),
            )
        });
        sd_handler
            .expect_create_availability_callback()
            .times(1)
            .returning(|_| AvailabilityCallback::create(Vec::new(), Vec::new()));

        someip_context_factory
            .expect_create()
            .times(1)
            .returning(move |_, _, _| Ok(Box::new(MockISomeIpStackContext::new())));

        pubsub_handler.expect_start().times(1).returning(|_, _| Ok(()));
        rpc_handler.expect_start().times(1).returning(|_, _| Ok(()));
        sd_handler.expect_start().times(1).return_const(());
    }

    #[tokio::test]
    async fn broker_start_populates_pubsub_handler_correctly() {
        let mut someip_context_factory = MockISomeIpStackContextFactory::new();
        let mut availability_message_factory = MockIAvailabilityMessageFactory::new();
        let mut pubsub_handler = MockIPubSubHandler::new();
        let mut rpc_handler = MockIRpcHandler::new();
        let mut sd_handler = MockIServiceDiscoveryHandler::new();

        set_default_expectations(
            &mut someip_context_factory,
            &mut availability_message_factory,
            &mut pubsub_handler,
            &mut rpc_handler,
            &mut sd_handler,
        );

        let mut protocol_mapping = ProtocolMapping::default();
        let simple_topic_mapping =
            SdvTopicMappingInfo::SimpleMapping { someip_event_id: 3, someip_eventgroup_id: 15 };
        let service_unit_interface = SdvServiceUnitInterface::TOPIC(simple_topic_mapping.clone());

        let sdv_service_unit_mapping =
            create_default_sdv_service_mapping(service_unit_interface.clone());
        protocol_mapping.sdv_service_units.push(sdv_service_unit_mapping.clone());

        let SdvIdentifier::ServiceUnitInfo { service_unit_id } =
            sdv_service_unit_mapping.sdv_identifier
        else {
            panic!("Unexpected identifier type")
        };

        let event_mapping_info = create_someip_event_mapping(
            1,
            2,
            SomeIpEventMappingType::SIMPLE(Box::new(BrokerServiceUnit {
                service_unit_name: "test_service_name".to_string(),
                unit_type: UnitType::builder()
                    .set_sdv_package_name("com.test.package")
                    .set_service_bundle_name("TestBundle")
                    .set_unit_type_name("TestUnitType")
                    .build()
                    .expect("Unable to build unit type"),
            })),
        );
        let someip_service_mapping =
            create_default_someip_service_mapping(vec![event_mapping_info.clone()], Vec::new());
        protocol_mapping.someip_service_instances.push(someip_service_mapping.clone());

        pubsub_handler
            .expect_add_topic_mapping()
            .with(
                eq(SdvIdentifier::ServiceUnitInfo { service_unit_id }),
                eq(sdv_service_unit_mapping.someip_service_instance_info),
                eq(simple_topic_mapping),
            )
            .times(1)
            .return_const(());

        pubsub_handler
            .expect_add_event_mapping()
            .with(
                eq(someip_service_mapping.someip_service_instance_info),
                eq(vec![event_mapping_info]),
            )
            .times(1)
            .return_const(());

        sd_handler.expect_add_sdv_handle().times(1).return_const(());
        sd_handler.expect_add_someip_handle().times(1).return_const(());

        let mut broker = SomeIpBroker {
            someip_context_factory: Box::new(someip_context_factory),
            protocol_mapping,
            sd_handler: Arc::new(sd_handler),
            pubsub_handler: Arc::new(pubsub_handler),
            rpc_handler: Arc::new(rpc_handler),
            availability_message_factory: Box::new(availability_message_factory),
        };
        let _ = broker.start(None).await;
    }

    #[tokio::test]
    async fn broker_start_populates_rpc_handler_correctly() {
        let mut someip_context_factory = MockISomeIpStackContextFactory::new();
        let mut availability_message_factory = MockIAvailabilityMessageFactory::new();
        let mut pubsub_handler = MockIPubSubHandler::new();
        let mut rpc_handler = MockIRpcHandler::new();
        let mut sd_handler = MockIServiceDiscoveryHandler::new();

        set_default_expectations(
            &mut someip_context_factory,
            &mut availability_message_factory,
            &mut pubsub_handler,
            &mut rpc_handler,
            &mut sd_handler,
        );

        let mut protocol_mapping = ProtocolMapping::default();
        let sdv_rpc_method1 = RpcMethodMappingInfo {
            sdv_method_name: "sdv_rpc_method1".to_string(),
            someip_method_id: 1,
        };
        let sdv_rpc_method2 = RpcMethodMappingInfo {
            sdv_method_name: "sdv_rpc_method2".to_string(),
            someip_method_id: 2,
        };
        let sdv_rpc_method_infos = vec![sdv_rpc_method1, sdv_rpc_method2];

        let service_unit_interface = SdvServiceUnitInterface::RPC(sdv_rpc_method_infos.clone());

        let sdv_service_unit_mapping =
            create_default_sdv_service_mapping(service_unit_interface.clone());
        protocol_mapping.sdv_service_units.push(sdv_service_unit_mapping.clone());

        let someip_rpc_method1 = RpcMethodMappingInfo {
            sdv_method_name: "someip_rpc_method1".to_string(),
            someip_method_id: 10,
        };
        let someip_rpc_method2 = RpcMethodMappingInfo {
            sdv_method_name: "someip_rpc_method2".to_string(),
            someip_method_id: 20,
        };
        let broker_service_unit = BrokerServiceUnit {
            service_unit_name: "service_unit_name".to_string(),
            unit_type: sdv_service_unit_mapping.unit_type.clone(),
        };

        let someip_rpc_mapping_info = SomeIpRpcMappingInfo {
            broker_service_unit,
            rpc_method_infos: vec![someip_rpc_method1.clone(), someip_rpc_method2.clone()],
        };
        let someip_service_mapping = create_default_someip_service_mapping(
            Vec::new(),
            vec![someip_rpc_mapping_info.clone()],
        );
        protocol_mapping.someip_service_instances.push(someip_service_mapping.clone());

        let SdvIdentifier::ServiceUnitInfo { service_unit_id } =
            &sdv_service_unit_mapping.sdv_identifier
        else {
            panic!("Unexpected identifier type")
        };

        rpc_handler
            .expect_add_sdv_rpc_mapping()
            .with(
                eq(SdvIdentifier::ServiceUnitInfo { service_unit_id: service_unit_id.clone() }),
                eq(sdv_service_unit_mapping.someip_service_instance_info),
                eq(sdv_rpc_method_infos),
            )
            .times(1)
            .return_const(());

        rpc_handler
            .expect_add_someip_rpc_mapping()
            .with(
                eq(someip_service_mapping.someip_service_instance_info),
                eq(vec![someip_rpc_mapping_info]),
            )
            .times(1)
            .return_const(());

        sd_handler.expect_add_sdv_handle().times(1).return_const(());
        sd_handler.expect_add_someip_handle().times(1).return_const(());

        let mut broker = SomeIpBroker {
            someip_context_factory: Box::new(someip_context_factory),
            protocol_mapping,
            sd_handler: Arc::new(sd_handler),
            pubsub_handler: Arc::new(pubsub_handler),
            rpc_handler: Arc::new(rpc_handler),
            availability_message_factory: Box::new(availability_message_factory),
        };
        let _ = broker.start(None).await;
    }

    #[tokio::test]
    async fn broker_start_populates_sd_handler_correctly() {
        let mut someip_context_factory = MockISomeIpStackContextFactory::new();
        let mut availability_message_factory = MockIAvailabilityMessageFactory::new();
        let mut pubsub_handler = MockIPubSubHandler::new();
        let mut rpc_handler = MockIRpcHandler::new();
        let mut sd_handler = MockIServiceDiscoveryHandler::new();

        set_default_expectations(
            &mut someip_context_factory,
            &mut availability_message_factory,
            &mut pubsub_handler,
            &mut rpc_handler,
            &mut sd_handler,
        );

        let mut protocol_mapping = ProtocolMapping::default();

        let service_unit_interface = SdvServiceUnitInterface::RPC(vec![]);

        let sdv_service_unit_mapping =
            create_default_sdv_service_mapping(service_unit_interface.clone());
        protocol_mapping.sdv_service_units.push(sdv_service_unit_mapping.clone());

        let broker_service_unit = BrokerServiceUnit {
            service_unit_name: "service_unit_name".to_string(),
            unit_type: sdv_service_unit_mapping.unit_type.clone(),
        };

        let someip_rpc_mapping_info =
            SomeIpRpcMappingInfo { broker_service_unit, rpc_method_infos: vec![] };
        let someip_service_mapping = create_default_someip_service_mapping(
            Vec::new(),
            vec![someip_rpc_mapping_info.clone()],
        );
        protocol_mapping.someip_service_instances.push(someip_service_mapping.clone());

        rpc_handler.expect_add_sdv_rpc_mapping().times(1).return_const(());
        rpc_handler.expect_add_someip_rpc_mapping().times(1).return_const(());

        let SdvIdentifier::ServiceUnitInfo { service_unit_id } =
            sdv_service_unit_mapping.sdv_identifier
        else {
            panic!("Unexpected identifier type")
        };

        sd_handler
            .expect_add_sdv_handle()
            .with(
                eq(SdvIdentifier::ServiceUnitInfo { service_unit_id }),
                eq(sdv_service_unit_mapping.unit_type),
            )
            .times(1)
            .return_const(());
        sd_handler
            .expect_add_someip_handle()
            .with(eq(someip_service_mapping.someip_service_instance_info))
            .times(1)
            .return_const(());

        let mut broker = SomeIpBroker {
            someip_context_factory: Box::new(someip_context_factory),
            protocol_mapping,
            sd_handler: Arc::new(sd_handler),
            pubsub_handler: Arc::new(pubsub_handler),
            rpc_handler: Arc::new(rpc_handler),
            availability_message_factory: Box::new(availability_message_factory),
        };
        let _ = broker.start(None).await;
    }

    fn create_default_sdv_soa_service_mapping(
        service_unit_interface: SdvServiceUnitInterface,
    ) -> SdvServiceUnitMapping {
        SdvServiceUnitMapping {
            sdv_identifier: SdvIdentifier::SoaInfo {
                soa_identifier: "test-soa-identifier".to_string(),
            },
            unit_type: UnitType::builder()
                .set_sdv_package_name("com.test.package")
                .set_service_bundle_name("TestBundle")
                .set_unit_type_name("TestUnitType")
                .build()
                .unwrap(),
            someip_service_instance_info: SomeIpService {
                service_id: 2,
                instance_id: 4,
                version: SomeIpServiceVersion { major: 2, minor: 1 },
            },
            service_unit_interface,
        }
    }

    #[tokio::test]
    async fn broker_start_populates_pubsub_handler_correctly_soa() {
        let mut someip_context_factory = MockISomeIpStackContextFactory::new();
        let mut availability_message_factory = MockIAvailabilityMessageFactory::new();
        let mut pubsub_handler = MockIPubSubHandler::new();
        let mut rpc_handler = MockIRpcHandler::new();
        let mut sd_handler = MockIServiceDiscoveryHandler::new();

        set_default_expectations(
            &mut someip_context_factory,
            &mut availability_message_factory,
            &mut pubsub_handler,
            &mut rpc_handler,
            &mut sd_handler,
        );

        let mut protocol_mapping = ProtocolMapping::default();
        let simple_topic_mapping =
            SdvTopicMappingInfo::SimpleMapping { someip_event_id: 3, someip_eventgroup_id: 15 };
        let service_unit_interface = SdvServiceUnitInterface::TOPIC(simple_topic_mapping.clone());

        let sdv_service_unit_mapping =
            create_default_sdv_soa_service_mapping(service_unit_interface.clone());
        protocol_mapping.sdv_service_units.push(sdv_service_unit_mapping.clone());

        let SdvIdentifier::SoaInfo { soa_identifier } =
            sdv_service_unit_mapping.sdv_identifier.clone()
        else {
            panic!("Unexpected identifier type")
        };

        let event_mapping_info = create_someip_event_mapping(
            1,
            2,
            SomeIpEventMappingType::SIMPLE(Box::new(BrokerServiceUnit {
                service_unit_name: soa_identifier.to_string(),
                unit_type: UnitType::builder()
                    .set_sdv_package_name("com.test.package")
                    .set_service_bundle_name("TestBundle")
                    .set_unit_type_name("TestUnitType")
                    .build()
                    .expect("Unable to build unit type"),
            })),
        );
        let someip_service_mapping =
            create_default_someip_service_mapping(vec![event_mapping_info.clone()], Vec::new());
        protocol_mapping.someip_service_instances.push(someip_service_mapping.clone());

        pubsub_handler
            .expect_add_topic_mapping()
            .with(
                eq(SdvIdentifier::SoaInfo { soa_identifier }),
                eq(sdv_service_unit_mapping.someip_service_instance_info),
                eq(simple_topic_mapping),
            )
            .times(1)
            .return_const(());

        pubsub_handler
            .expect_add_event_mapping()
            .with(
                eq(someip_service_mapping.someip_service_instance_info),
                eq(vec![event_mapping_info]),
            )
            .times(1)
            .return_const(());

        sd_handler.expect_add_sdv_handle().times(1).return_const(());
        sd_handler.expect_add_someip_handle().times(1).return_const(());

        let mut broker = SomeIpBroker {
            someip_context_factory: Box::new(someip_context_factory),
            protocol_mapping,
            sd_handler: Arc::new(sd_handler),
            pubsub_handler: Arc::new(pubsub_handler),
            rpc_handler: Arc::new(rpc_handler),
            availability_message_factory: Box::new(availability_message_factory),
        };
        let _ = broker.start(None).await;
    }

    #[tokio::test]
    async fn broker_start_populates_rpc_handler_correctly_soa() {
        let mut someip_context_factory = MockISomeIpStackContextFactory::new();
        let mut availability_message_factory = MockIAvailabilityMessageFactory::new();
        let mut pubsub_handler = MockIPubSubHandler::new();
        let mut rpc_handler = MockIRpcHandler::new();
        let mut sd_handler = MockIServiceDiscoveryHandler::new();

        set_default_expectations(
            &mut someip_context_factory,
            &mut availability_message_factory,
            &mut pubsub_handler,
            &mut rpc_handler,
            &mut sd_handler,
        );

        let mut protocol_mapping = ProtocolMapping::default();
        let sdv_rpc_method1 = RpcMethodMappingInfo {
            sdv_method_name: "sdv_rpc_method1".to_string(),
            someip_method_id: 1,
        };
        let sdv_rpc_method2 = RpcMethodMappingInfo {
            sdv_method_name: "sdv_rpc_method2".to_string(),
            someip_method_id: 2,
        };
        let sdv_rpc_method_infos = vec![sdv_rpc_method1, sdv_rpc_method2];

        let service_unit_interface = SdvServiceUnitInterface::RPC(sdv_rpc_method_infos.clone());

        let sdv_service_unit_mapping =
            create_default_sdv_soa_service_mapping(service_unit_interface.clone());
        protocol_mapping.sdv_service_units.push(sdv_service_unit_mapping.clone());

        let SdvIdentifier::SoaInfo { soa_identifier } =
            sdv_service_unit_mapping.sdv_identifier.clone()
        else {
            panic!("Unexpected identifier type")
        };

        let someip_rpc_method1 = RpcMethodMappingInfo {
            sdv_method_name: "someip_rpc_method1".to_string(),
            someip_method_id: 10,
        };
        let someip_rpc_method2 = RpcMethodMappingInfo {
            sdv_method_name: "someip_rpc_method2".to_string(),
            someip_method_id: 20,
        };
        let broker_service_unit = BrokerServiceUnit {
            service_unit_name: soa_identifier.clone(),
            unit_type: sdv_service_unit_mapping.unit_type.clone(),
        };

        let someip_rpc_mapping_info = SomeIpRpcMappingInfo {
            broker_service_unit,
            rpc_method_infos: vec![someip_rpc_method1.clone(), someip_rpc_method2.clone()],
        };
        let someip_service_mapping = create_default_someip_service_mapping(
            Vec::new(),
            vec![someip_rpc_mapping_info.clone()],
        );
        protocol_mapping.someip_service_instances.push(someip_service_mapping.clone());

        rpc_handler
            .expect_add_sdv_rpc_mapping()
            .with(
                eq(SdvIdentifier::SoaInfo { soa_identifier }),
                eq(sdv_service_unit_mapping.someip_service_instance_info),
                eq(sdv_rpc_method_infos),
            )
            .times(1)
            .return_const(());

        rpc_handler
            .expect_add_someip_rpc_mapping()
            .with(
                eq(someip_service_mapping.someip_service_instance_info),
                eq(vec![someip_rpc_mapping_info]),
            )
            .times(1)
            .return_const(());

        sd_handler.expect_add_sdv_handle().times(1).return_const(());
        sd_handler.expect_add_someip_handle().times(1).return_const(());

        let mut broker = SomeIpBroker {
            someip_context_factory: Box::new(someip_context_factory),
            protocol_mapping,
            sd_handler: Arc::new(sd_handler),
            pubsub_handler: Arc::new(pubsub_handler),
            rpc_handler: Arc::new(rpc_handler),
            availability_message_factory: Box::new(availability_message_factory),
        };
        let _ = broker.start(None).await;
    }

    #[tokio::test]
    async fn broker_start_populates_sd_handler_correctly_soa() {
        let mut someip_context_factory = MockISomeIpStackContextFactory::new();
        let mut availability_message_factory = MockIAvailabilityMessageFactory::new();
        let mut pubsub_handler = MockIPubSubHandler::new();
        let mut rpc_handler = MockIRpcHandler::new();
        let mut sd_handler = MockIServiceDiscoveryHandler::new();

        set_default_expectations(
            &mut someip_context_factory,
            &mut availability_message_factory,
            &mut pubsub_handler,
            &mut rpc_handler,
            &mut sd_handler,
        );

        let mut protocol_mapping = ProtocolMapping::default();

        let service_unit_interface = SdvServiceUnitInterface::RPC(vec![]);

        let sdv_service_unit_mapping =
            create_default_sdv_soa_service_mapping(service_unit_interface.clone());
        protocol_mapping.sdv_service_units.push(sdv_service_unit_mapping.clone());

        let SdvIdentifier::SoaInfo { soa_identifier } =
            sdv_service_unit_mapping.sdv_identifier.clone()
        else {
            panic!("Unexpected identifier type")
        };

        let broker_service_unit = BrokerServiceUnit {
            service_unit_name: soa_identifier.clone(),
            unit_type: sdv_service_unit_mapping.unit_type.clone(),
        };

        let someip_rpc_mapping_info =
            SomeIpRpcMappingInfo { broker_service_unit, rpc_method_infos: vec![] };
        let someip_service_mapping = create_default_someip_service_mapping(
            Vec::new(),
            vec![someip_rpc_mapping_info.clone()],
        );
        protocol_mapping.someip_service_instances.push(someip_service_mapping.clone());

        rpc_handler.expect_add_sdv_rpc_mapping().times(1).return_const(());
        rpc_handler.expect_add_someip_rpc_mapping().times(1).return_const(());

        sd_handler
            .expect_add_sdv_handle()
            .with(
                eq(SdvIdentifier::SoaInfo { soa_identifier }),
                eq(sdv_service_unit_mapping.unit_type),
            )
            .times(1)
            .return_const(());
        sd_handler
            .expect_add_someip_handle()
            .with(eq(someip_service_mapping.someip_service_instance_info))
            .times(1)
            .return_const(());

        let mut broker = SomeIpBroker {
            someip_context_factory: Box::new(someip_context_factory),
            protocol_mapping,
            sd_handler: Arc::new(sd_handler),
            pubsub_handler: Arc::new(pubsub_handler),
            rpc_handler: Arc::new(rpc_handler),
            availability_message_factory: Box::new(availability_message_factory),
        };
        let _ = broker.start(None).await;
    }
}
