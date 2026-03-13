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

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::OnceLock;

use async_trait::async_trait;
use log::{error, info};
use mockall::automock;
use tokio_stream::{StreamExt, StreamMap};

use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::Client;
use sdv_comms::rpc::ClientOptions;
use sdv_comms::rpc::Server;
use sdv_comms::rpc::ServerOptions;
use sdv_comms::rpc::UnaryRpcMethod;
use sdv_comms::sd::ServiceDiscoveryManager;
use sdv_comms::sd::ServiceUnitChangeEvent;
use sdv_comms::sd::ServiceUnitDefinition;
use sdv_comms::sd::UnitType;
use sdv_comms::ContextRef;

use sdv_comms::dt::Publisher;
use sdv_comms::dt::Subscriber;
use sdv_comms::dt::SubscriberOptions;
use sdv_comms::id::UnitIdentifier;
use sdv_comms::sd::UnitRegistrationArgs;

use crate::agent::protocol_mapping::SdvIdentifier;
use crate::agent::AvailabilityMessage;
use crate::agent::IAvailabilityNotifier;
use crate::agent::ServiceType;

#[automock]
#[async_trait]
pub(crate) trait ISdvCommsContext: Sync + Send {
    async fn get_fqin(&self) -> ServiceFqin;

    /// Listen to changes in availability of unit types indicated by `service_unit_infos` and
    /// `multipub_types`, and trigger notification using `availability_notifiers`
    async fn listen_for_unit_type_availability_changes(
        &self,
        service_unit_infos: Vec<(SdvIdentifier, UnitType)>,
        multipub_types: Vec<UnitType>,
        availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
    );

    async fn create_publisher(
        &self,
        unit_registration_args: UnitRegistrationArgs,
    ) -> anyhow::Result<Publisher>;

    async fn create_subscriber(
        &self,
        unit_identifier: &UnitIdentifier,
    ) -> anyhow::Result<Subscriber>;

    async fn create_multipub_subscriber(&self, unit_type: &UnitType) -> anyhow::Result<Subscriber>;

    async fn create_server(
        &self,
        unit_registration_args: UnitRegistrationArgs,
        unary_methods_map: HashMap<String, UnaryRpcMethod>,
    ) -> anyhow::Result<Server>;

    async fn create_client(&self, unit_identifier: UnitIdentifier) -> anyhow::Result<Client>;
}

static COMM_STACK_AGENTS_WAITER: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

#[derive(Clone)]
pub(crate) struct SdvCommsContext {
    comms_context: OnceLock<ContextRef>,
    raw_fqin: ServiceFqin,
}

impl SdvCommsContext {
    pub(crate) fn create(raw_fqin: ServiceFqin) -> SdvCommsContext {
        SdvCommsContext { comms_context: OnceLock::new(), raw_fqin }
    }

    async fn notify(
        &self,
        availability_notifiers: &Vec<Box<dyn IAvailabilityNotifier>>,
        sud: ServiceUnitDefinition,
        available: bool,
    ) {
        for availability_notifier in availability_notifiers {
            if let Err(e) = availability_notifier
                .notify(AvailabilityMessage {
                    service: ServiceType::SDV(Box::new(sud.clone())),
                    available,
                })
                .await
            {
                error!("Failed to notify listener about availability change of SDV service {sud:?}. Error {e:?}");
            }
        }
    }

    async fn trigger_availability_notifiers(
        &self,
        sud: ServiceUnitDefinition,
        availability_notifiers: &Vec<Box<dyn IAvailabilityNotifier>>,
        sdv_identifiers_by_type: &HashMap<UnitType, Vec<SdvIdentifier>>,
        multipub_types: &HashSet<UnitType>,
        available: bool,
    ) {
        let unit_type = sud.get_unit_type();

        let Some(sdv_identifiers) = sdv_identifiers_by_type.get(&unit_type) else {
            if multipub_types.contains(&unit_type) {
                self.notify(availability_notifiers, sud, available).await;
            } else {
                info!("Couldn't find service units for Unit Type {unit_type}");
            }
            return;
        };

        let unit_identifier = sud.get_unit_identifier();
        let registered_service_unit_name = unit_identifier.get_unit_name();
        let registered_service_fqin = sud.get_service_bundle_identity().get_fqin().clone();

        if sdv_identifiers.iter().any(|sdv_identifier| match sdv_identifier {
            SdvIdentifier::ServiceUnitInfo { service_unit_id } => {
                service_unit_id.service_unit_name == registered_service_unit_name
                    && service_unit_id.service_fqin.matches(&registered_service_fqin)
            }
            SdvIdentifier::SoaInfo { soa_identifier: _ } => {
                false
                // TODO: b/487204223 - implement SOA logic in SOME/IP broker
            }
        }) {
            self.notify(availability_notifiers, sud, available).await;
        }
    }

    async fn get_context(&self) -> ContextRef {
        COMM_STACK_AGENTS_WAITER
            .get_or_init(async || {
                sdv_comms_ctx::wait_for_agents_async()
                    .await
                    .expect("Error waiting for Comm Stack agents - Error {e:?}");
            })
            .await;
        *self.comms_context.get_or_init(|| ContextRef::create(self.raw_fqin.clone()))
    }
}

#[async_trait]
impl ISdvCommsContext for SdvCommsContext {
    async fn get_fqin(&self) -> ServiceFqin {
        self.get_context().await.get_self_fqin()
    }

    async fn listen_for_unit_type_availability_changes(
        &self,
        service_unit_infos: Vec<(SdvIdentifier, UnitType)>,
        multipub_types: Vec<UnitType>,
        availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
    ) {
        let service_discovery_manager = ServiceDiscoveryManager::new(self.get_context().await);

        let mut sdv_identifiers_by_type = HashMap::new();
        for (sdv_identifier, unit_type) in service_unit_infos {
            let identifier_clone = sdv_identifier.clone();
            sdv_identifiers_by_type
                .entry(unit_type)
                .and_modify(|sdv_identifiers: &mut Vec<SdvIdentifier>| {
                    sdv_identifiers.push(identifier_clone)
                })
                .or_insert(vec![sdv_identifier]);
        }
        let multipub_types_set = HashSet::from_iter(multipub_types);

        let mut stream_map = StreamMap::new();
        for unit_type in sdv_identifiers_by_type.keys() {
            if let Ok(unit_type_stream) =
                service_discovery_manager.subscribe_service_unit_change_by_type(unit_type).await
            {
                stream_map.insert(unit_type.clone(), unit_type_stream);
            } else {
                error!(
                    "Failed to get listener for availability changes for Unit Type {unit_type:?}"
                );
            };
        }
        for unit_type in &multipub_types_set {
            if let Ok(unit_type_stream) =
                service_discovery_manager.subscribe_service_unit_change_by_type(unit_type).await
            {
                stream_map.insert(unit_type.clone(), unit_type_stream);
            } else {
                error!(
                    "Failed to get listener for availability changes for Unit Type {unit_type:?}"
                );
            };
        }

        while let Some((_, unit_change_event)) = stream_map.next().await {
            match unit_change_event {
                ServiceUnitChangeEvent::Registered(sud) => {
                    self.trigger_availability_notifiers(
                        sud,
                        &availability_notifiers,
                        &sdv_identifiers_by_type,
                        &multipub_types_set,
                        true,
                    )
                    .await;
                }
                ServiceUnitChangeEvent::Unregistered(sud) => {
                    self.trigger_availability_notifiers(
                        sud,
                        &availability_notifiers,
                        &sdv_identifiers_by_type,
                        &multipub_types_set,
                        false,
                    )
                    .await;
                }
                ServiceUnitChangeEvent::Changed(sud) => {
                    info!("Service Unit Definition change for {sud:?}")
                }
            }
        }
    }

    async fn create_publisher(
        &self,
        unit_registration_args: UnitRegistrationArgs,
    ) -> anyhow::Result<Publisher> {
        let unit_name = unit_registration_args.get_unit_name().clone();
        Publisher::create(self.get_context().await, unit_registration_args).await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to create publisher with service_unit_name {}: {e:?}",
                unit_name
            )
        })
    }

    async fn create_multipub_subscriber(&self, unit_type: &UnitType) -> anyhow::Result<Subscriber> {
        Subscriber::create_for_multipub(
            self.get_context().await,
            unit_type,
            SubscriberOptions { fetch_last_message: true },
        )
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to create subscriber for multipub unit type {}: {e:?}",
                unit_type
            )
        })
    }

    async fn create_subscriber(
        &self,
        unit_identifier: &UnitIdentifier,
    ) -> anyhow::Result<Subscriber> {
        Subscriber::create(
            self.get_context().await,
            unit_identifier,
            SubscriberOptions { fetch_last_message: true },
        )
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to create subscriber for service unit name {}: {e:?}",
                unit_identifier.get_unit_name()
            )
        })
    }

    async fn create_server(
        &self,
        unit_registration_args: UnitRegistrationArgs,
        unary_methods_map: HashMap<String, UnaryRpcMethod>,
    ) -> anyhow::Result<Server> {
        Server::create(
            self.get_context().await,
            unit_registration_args,
            unary_methods_map,
            &ServerOptions::default(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create Server: {e:?}"))
    }

    async fn create_client(&self, unit_identifier: UnitIdentifier) -> anyhow::Result<Client> {
        Client::create(self.get_context().await, unit_identifier, &ClientOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Client: {e:?}"))
    }
}
