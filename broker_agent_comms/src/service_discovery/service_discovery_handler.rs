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
use log::error;
use mockall::automock;
use std::sync::Arc;

use sdv_comms::sd::UnitType;

use crate::agent::protocol_mapping::SdvServiceUnitInfo;
use crate::agent::protocol_mapping::SomeIpServiceInstanceInfo;
use crate::agent::IAvailabilityNotifier;
use crate::comms::ISdvCommsContext;
use crate::comms::ISomeIpStackContext;
use crate::service_discovery::AvailabilityCallback;

#[automock]
#[async_trait]
pub(crate) trait IServiceDiscoveryHandler: Send + Sync {
    fn add_sdv_handle(&mut self, sdv_handle: SdvServiceUnitInfo);
    fn add_sdv_multipub_handle(&mut self, unit_type: UnitType);
    fn add_someip_handle(&mut self, someip_handle: SomeIpServiceInstanceInfo);
    fn create_availability_callback(
        &self,
        availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
    ) -> AvailabilityCallback;
    async fn start(
        &self,
        availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
        someip_comms: Arc<dyn ISomeIpStackContext>,
    );
}

pub(crate) struct ServiceDiscoveryHandler {
    sdv_comms: Arc<dyn ISdvCommsContext>,
    sdv_service_units: Vec<SdvServiceUnitInfo>,
    sdv_multipub_types: Vec<UnitType>,
    someip_service_instances: Vec<SomeIpServiceInstanceInfo>,
}

impl ServiceDiscoveryHandler {
    pub(crate) fn new(sdv_comms: Arc<dyn ISdvCommsContext>) -> Self {
        ServiceDiscoveryHandler {
            sdv_comms,
            sdv_service_units: Vec::new(),
            sdv_multipub_types: Vec::new(),
            someip_service_instances: Vec::new(),
        }
    }
}

#[async_trait]
impl IServiceDiscoveryHandler for ServiceDiscoveryHandler {
    fn add_sdv_handle(&mut self, sdv_handle: SdvServiceUnitInfo) {
        self.sdv_service_units.push(sdv_handle);
    }

    fn add_sdv_multipub_handle(&mut self, unit_type: UnitType) {
        self.sdv_multipub_types.push(unit_type);
    }

    fn add_someip_handle(&mut self, someip_handle: SomeIpServiceInstanceInfo) {
        self.someip_service_instances.push(someip_handle);
    }

    fn create_availability_callback(
        &self,
        availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
    ) -> AvailabilityCallback {
        AvailabilityCallback::create(self.someip_service_instances.clone(), availability_notifiers)
    }

    async fn start(
        &self,
        availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
        someip_comms: Arc<dyn ISomeIpStackContext>,
    ) {
        for someip_service in &self.someip_service_instances {
            if let Err(e) =
                someip_comms.monitor_service(&someip_service.someip_service_instance).await
            {
                error!(
                    "Error notifying SOME/IP stack to monitor service {:?} - Error {e:?}",
                    someip_service.someip_service_instance
                );
            }
        }

        self.sdv_comms
            .listen_for_unit_type_availability_changes(
                self.sdv_service_units.clone(),
                self.sdv_multipub_types.clone(),
                availability_notifiers,
            )
            .await;
    }
}

#[cfg(test)]
mod tests {

    use sdv_comms::sd::UnitType;
    use someip_config::broker_params::SdMinorVersionMode;
    use std::sync::Arc;

    use crate::agent::protocol_mapping::SdvServiceUnitId;
    use crate::agent::protocol_mapping::SdvServiceUnitInfo;
    use crate::agent::protocol_mapping::ServiceFqinInternal;
    use crate::agent::protocol_mapping::SomeIpServiceInstanceInfo;
    use crate::agent::MockIAvailabilityNotifier;

    use crate::comms::{MockISdvCommsContext, MockISomeIpStackContext};
    use crate::service_discovery::{IServiceDiscoveryHandler, ServiceDiscoveryHandler};

    use google_sdv_someip::aidl::google::sdv::someip::{
        SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
    };

    #[tokio::test]
    async fn test_add_sdv_handle_registers_for_listen() {
        let mut mock_comms = MockISdvCommsContext::new();
        let mut mock_availability_notifier = MockIAvailabilityNotifier::new();
        mock_availability_notifier
            .expect_clone_box()
            .returning(|| Box::new(MockIAvailabilityNotifier::new()));

        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.test_package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unable to build unit type");

        let service_unit_name = String::from("test_unit_name");
        let service_fqin = ServiceFqinInternal {
            sdv_vm_name: String::from("test_vm_name").into(),
            sdv_package_name: String::from("com.test_package"),
            service_bundle_name: String::from("TestBundle"),
            service_instance_name: String::from("test_instance_name").into(),
        };

        let service_unit_id = SdvServiceUnitId { service_fqin, service_unit_name };
        let sdv_handle = SdvServiceUnitInfo { service_unit_id, unit_type };
        let sdv_handle_clone = sdv_handle.clone();

        mock_comms
            .expect_listen_for_unit_type_availability_changes()
            .withf(move |service_units, multipub_types, notifiers| {
                multipub_types.is_empty()
                    && service_units.len() == 1
                    && notifiers.len() == 1
                    && service_units[0] == sdv_handle_clone
            })
            .times(1)
            .return_const(());

        let mut discovery_handler = ServiceDiscoveryHandler::new(Arc::new(mock_comms));

        discovery_handler.add_sdv_handle(sdv_handle);
        discovery_handler
            .start(
                vec![Box::new(mock_availability_notifier)],
                Arc::new(MockISomeIpStackContext::new()),
            )
            .await;
    }

    #[tokio::test]
    async fn test_add_sdv_multipub_handle_registers_for_listen() {
        let mut mock_comms = MockISdvCommsContext::new();
        let mut mock_availability_notifier = MockIAvailabilityNotifier::new();
        mock_availability_notifier
            .expect_clone_box()
            .returning(|| Box::new(MockIAvailabilityNotifier::new()));

        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.test_package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unable to build unit type");

        let sdv_multipub_handle = unit_type.clone();

        mock_comms
            .expect_listen_for_unit_type_availability_changes()
            .withf(move |service_units, multipub_types, notifiers| {
                service_units.is_empty()
                    && multipub_types.len() == 1
                    && notifiers.len() == 1
                    && multipub_types[0] == sdv_multipub_handle
            })
            .times(1)
            .return_const(());

        let mut discovery_handler = ServiceDiscoveryHandler::new(Arc::new(mock_comms));

        discovery_handler.add_sdv_multipub_handle(unit_type);
        discovery_handler
            .start(
                vec![Box::new(mock_availability_notifier)],
                Arc::new(MockISomeIpStackContext::new()),
            )
            .await;
    }

    #[tokio::test]
    async fn test_add_someip_handle_registers_for_listen() {
        let mut mock_comms = MockISdvCommsContext::new();
        let mut mock_stack = MockISomeIpStackContext::new();
        let service = SomeIpService {
            service_id: 1,
            instance_id: 1,
            version: SomeIpServiceVersion { major: 1, minor: 0 },
        };

        mock_stack
            .expect_monitor_service()
            .withf(move |someip_service| *someip_service == service)
            .times(1)
            .returning(|_| Ok(()));
        mock_comms.expect_listen_for_unit_type_availability_changes().times(1).return_const(());

        let mut discovery_handler = ServiceDiscoveryHandler::new(Arc::new(mock_comms));
        let someip_service_instance_info =
            SomeIpServiceInstanceInfo::new(service, SdMinorVersionMode::ExactOrAnyMinorVersion);
        discovery_handler.add_someip_handle(someip_service_instance_info);

        discovery_handler.start(Vec::new(), Arc::new(mock_stack)).await;
    }
}
