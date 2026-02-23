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

use sdv_comms::{
    id::ServiceFqin,
    sd::{ServiceUnitDefinition, UnitType},
};

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;
use someip_config::broker_params::SdMinorVersionMode;
use someip_serde_reflect::SomeIpStructTypeDefinition;

#[derive(Default)]
pub(crate) struct BrokerMapping {
    pub protocol_mapping: ProtocolMapping,
    pub type_mapping: TypeMapping,
}

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub(crate) enum MessageTranslationMode {
    #[default]
    Reflection,
    DynamicLibrary,
    InterpretAsBytes,
}

#[derive(Default, Clone)]
pub(crate) struct SomeIpTypeMapping {
    type_mapping: Option<SomeIpStructTypeDefinition>,
    message_translation_mode: MessageTranslationMode,
}

impl SomeIpTypeMapping {
    pub(crate) fn new(type_mapping: Option<SomeIpStructTypeDefinition>) -> SomeIpTypeMapping {
        Self::with_mode(type_mapping, MessageTranslationMode::Reflection)
    }

    pub(crate) fn with_mode(
        type_mapping: Option<SomeIpStructTypeDefinition>,
        message_translation_mode: MessageTranslationMode,
    ) -> SomeIpTypeMapping {
        SomeIpTypeMapping { type_mapping, message_translation_mode }
    }

    pub(crate) fn message_translation_mode(&self) -> MessageTranslationMode {
        self.message_translation_mode
    }

    pub(crate) fn type_mapping(&self) -> Option<&SomeIpStructTypeDefinition> {
        self.type_mapping.as_ref()
    }
}

#[derive(Default)]
pub(crate) struct TypeMapping {
    pub topic_type_mapping: HashMap<UnitType, SomeIpTypeMapping>,
    pub rpc_type_mapping:
        HashMap<UnitType, HashMap<String, (SomeIpTypeMapping, SomeIpTypeMapping)>>,
}

#[derive(Default)]
pub(crate) struct ProtocolMapping {
    pub sdv_service_units: Vec<SdvServiceUnitMapping>,
    pub sdv_multipub_topics: Vec<SdvMultipubMapping>,
    pub someip_service_instances: Vec<SomeIpServiceInstanceMapping>,
}

#[derive(PartialEq, Clone, Debug)]
pub(crate) struct SdvMultipubMapping {
    pub unit_type: UnitType,
    pub someip_service_instance_info: SomeIpService,
    pub someip_event_id: u16,
    pub someip_eventgroup_id: u16,
}

/// Identifies a mapping between an internal SDV service and external SOME/IP service
///
/// - SdvIdentifier points to SDV service (specific or Topic/Channel in SOA)
/// - unit_type points to the proto messaged provided by the sdv_identifier
/// - someip_service_instance_info
/// - service_unit_interface
#[derive(PartialEq, Clone)]
pub(crate) struct SdvServiceUnitMapping {
    pub sdv_identifier: SdvIdentifier,
    pub unit_type: UnitType,
    pub someip_service_instance_info: SomeIpService,
    pub service_unit_interface: SdvServiceUnitInterface,
}

/// Identifies a service in SDV environment that needs to be exported to SOME/IP
///
/// It can be either:
/// - ServiceUnitInfo: a specific instance of an SDV service unit (Legacy)
///   identified by FQIN + service_unit_name
/// - SoaInfo:
///   - a reference to a PubSub Topic
///   - a reference to a RPC Channel
///
/// In the SOA scenario, the Type used by this unit is stored
/// inside SdvServiceUnitMapping
#[derive(PartialEq, Hash, Eq, Clone, Debug)]
pub(crate) enum SdvIdentifier {
    ServiceUnitInfo {
        service_unit_id: SdvServiceUnitId,
    },
    #[allow(dead_code)]
    SoaInfo {
        soa_identifier: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SomeIpServiceInstanceInfo {
    pub someip_service_instance: SomeIpService,
    pub sd_minor_version_mode: SdMinorVersionMode,
}

impl SomeIpServiceInstanceInfo {
    pub(crate) fn new(
        someip_service_instance: SomeIpService,
        sd_minor_version_mode: SdMinorVersionMode,
    ) -> SomeIpServiceInstanceInfo {
        SomeIpServiceInstanceInfo { someip_service_instance, sd_minor_version_mode }
    }
}

#[derive(Clone, Default)]
pub(crate) struct SomeIpServiceInstanceMapping {
    pub someip_service_instance_info: SomeIpServiceInstanceInfo,
    pub event_infos: Vec<SomeIpEventMappingInfo>,
    pub rpc_infos: Vec<SomeIpRpcMappingInfo>,
}

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub(crate) struct ServiceFqinInternal {
    pub sdv_vm_name: Option<String>,
    pub sdv_package_name: String,
    pub service_bundle_name: String,
    pub service_instance_name: Option<String>,
}

impl ServiceFqinInternal {
    pub(crate) fn matches(&self, service_fqin: &ServiceFqin) -> bool {
        if self.sdv_package_name != service_fqin.get_sdv_package_name()
            || self.service_bundle_name != service_fqin.get_service_bundle_name()
        {
            false
        } else {
            match (&self.sdv_vm_name, &self.service_instance_name) {
                (None, None) => true,
                (None, Some(instance_name)) => {
                    instance_name == service_fqin.get_service_instance_name()
                }
                (Some(vm_name), None) => vm_name == service_fqin.get_sdv_vm_name(),
                (Some(vm_name), Some(instance_name)) => {
                    vm_name == service_fqin.get_sdv_vm_name()
                        && instance_name == service_fqin.get_service_instance_name()
                }
            }
        }
    }
}

/// This struct uniquely identifies a service unit within SDV using SDV Comm stack SDV structs
#[derive(PartialEq, Clone, Debug, Hash)]
pub(crate) struct SdvServiceUnitCommsId {
    pub service_fqin: ServiceFqin,
    pub service_unit_name: String,
}

impl SdvServiceUnitCommsId {
    pub(crate) fn from_service_unit_definition(
        service_unit_definition: &ServiceUnitDefinition,
    ) -> Self {
        SdvServiceUnitCommsId {
            service_fqin: service_unit_definition.get_service_bundle_identity().get_fqin().clone(),
            service_unit_name: service_unit_definition
                .get_unit_identifier()
                .get_unit_name()
                .to_owned(),
        }
    }
}

impl Eq for SdvServiceUnitCommsId {}

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub(crate) struct SdvServiceUnitId {
    pub service_fqin: ServiceFqinInternal,
    pub service_unit_name: String,
}

// Represents a specific service unit within the SDV communication framework
// that is being exposed by the SOME/IP broker. (SOME/IP -> [broker] -> SDV )
// This struct contains the necessary information to register or interact with
// this service unit via the `sdv_comms` interface.
#[derive(PartialEq, Clone, Debug)]
pub(crate) struct BrokerServiceUnit {
    // service_unit_name is set to service_unit value in case of legacy
    // service_unit_name is set to topic for import of PubSub
    // service_unit_name is set to channel for import of RPC
    pub service_unit_name: String,
    pub unit_type: UnitType,
}

/// Represents a unique SOME/IP method
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct SomeIpMethod {
    /// Service/Instance information about the server of the method
    pub someip_service: SomeIpService,
    /// Method ID
    pub someip_method_id: u16,
}

/// Represents a unique SOME/IP event
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct SomeIpEvent {
    /// Service/Instance information about the offerer of the event
    pub someip_service: SomeIpService,
    /// Event ID
    pub event_id: u16,
    /// Eventgroup ID
    pub eventgroup_id: u16,
}

impl SomeIpEvent {
    pub(crate) fn new(
        someip_service: SomeIpService,
        event_id: u16,
        eventgroup_id: u16,
    ) -> SomeIpEvent {
        SomeIpEvent { someip_service, event_id, eventgroup_id }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum SdvTopicMappingInfo {
    SimpleMapping { someip_event_id: u16, someip_eventgroup_id: u16 },
    FireAndForgetMapping { ff_mapping: Vec<SomeIpMethod> },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SomeIpRpcMappingInfo {
    pub broker_service_unit: BrokerServiceUnit,
    pub rpc_method_infos: Vec<RpcMethodMappingInfo>,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct RpcMethodMappingInfo {
    pub sdv_method_name: String,
    pub someip_method_id: u16,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum SomeIpEventMappingType {
    SIMPLE(Box<BrokerServiceUnit>),
    GROUP(Option<SomeIpStructTypeDefinition>, Vec<(BrokerServiceUnit, HashMap<String, String>)>),
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct SomeIpEventMappingInfo {
    pub event_id: u16,
    pub eventgroup_id: u16,
    pub event_mapping_type: SomeIpEventMappingType,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum SdvServiceUnitInterface {
    TOPIC(SdvTopicMappingInfo),
    RPC(Vec<RpcMethodMappingInfo>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_sdv_someip::aidl::google::sdv::someip::SomeIpServiceVersion::SomeIpServiceVersion;
    use sdv_comms::id::ServiceFqin;

    const PKG_NAME: &str = "com.example.package";
    const BUNDLE_NAME: &str = "ExampleBundle";
    const VM_NAME: &str = "vm";
    const INSTANCE_NAME: &str = "instance1";

    fn base_service_fqin_builder() -> ServiceFqin {
        ServiceFqin::builder()
            .sdv_vm_name(VM_NAME)
            .sdv_package_name(PKG_NAME)
            .service_bundle_name(BUNDLE_NAME)
            .service_instance_name(INSTANCE_NAME)
            .build()
            .unwrap()
    }

    #[test]
    fn test_service_fqin_internal_matches_package_name_mismatch() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some(VM_NAME.to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some(INSTANCE_NAME.to_string()),
        };
        let service_fqin = ServiceFqin::builder()
            .sdv_vm_name(VM_NAME)
            .sdv_package_name("com.different.package")
            .service_bundle_name(BUNDLE_NAME)
            .service_instance_name(INSTANCE_NAME)
            .build()
            .unwrap();
        assert!(!internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_bundle_name_mismatch() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some(VM_NAME.to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some(INSTANCE_NAME.to_string()),
        };
        let service_fqin = ServiceFqin::builder()
            .sdv_vm_name(VM_NAME)
            .sdv_package_name(PKG_NAME)
            .service_bundle_name("DifferentBundle")
            .service_instance_name(INSTANCE_NAME)
            .build()
            .unwrap();
        assert!(!internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_none_none() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: None,
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: None,
        };
        let service_fqin = base_service_fqin_builder(); // VM and Instance are set in builder
        assert!(internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_none_some_match() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: None,
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some(INSTANCE_NAME.to_string()),
        };
        let service_fqin = base_service_fqin_builder();
        assert!(internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_none_some_mismatch() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: None,
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some("DifferentInstance".to_string()),
        };
        let service_fqin = base_service_fqin_builder();
        assert!(!internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_some_none_match() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some(VM_NAME.to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: None,
        };
        let service_fqin = base_service_fqin_builder();
        assert!(internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_some_none_mismatch() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some("DifferentVM".to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: None,
        };
        let service_fqin = base_service_fqin_builder();
        assert!(!internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_some_some_match() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some(VM_NAME.to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some(INSTANCE_NAME.to_string()),
        };
        let service_fqin = base_service_fqin_builder();
        assert!(internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_some_some_vm_mismatch() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some("DifferentVM".to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some(INSTANCE_NAME.to_string()),
        };
        let service_fqin = base_service_fqin_builder();
        assert!(!internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_some_some_instance_mismatch() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some(VM_NAME.to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some("DifferentInstance".to_string()),
        };
        let service_fqin = base_service_fqin_builder();
        assert!(!internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_service_fqin_internal_matches_some_some_both_mismatch() {
        let internal_fqin = ServiceFqinInternal {
            sdv_vm_name: Some("DifferentVM".to_string()),
            sdv_package_name: PKG_NAME.to_string(),
            service_bundle_name: BUNDLE_NAME.to_string(),
            service_instance_name: Some("DifferentInstance".to_string()),
        };
        let service_fqin = base_service_fqin_builder();
        assert!(!internal_fqin.matches(&service_fqin));
    }

    #[test]
    fn test_someip_event_new_and_partial_eq() {
        let version = SomeIpServiceVersion { major: 1, minor: 0 };
        let service1 = SomeIpService { service_id: 10, instance_id: 1, version };
        let service2 = SomeIpService { service_id: 20, instance_id: 2, version }; // Different from service1

        let event_id1: u16 = 100;
        let eventgroup_id1: u16 = 200;

        let event_id2: u16 = 300; // Different from event_id1
        let eventgroup_id2: u16 = 400; // Different from eventgroup_id1

        // Create two identical SomeIpEvent instances
        let someip_event1 = SomeIpEvent::new(service1, event_id1, eventgroup_id1);
        let someip_event2 = SomeIpEvent {
            someip_service: service1,
            event_id: event_id1,
            eventgroup_id: eventgroup_id1,
        };

        // Create a different SomeIpEvent instance (different service_id)
        let someip_event3 = SomeIpEvent::new(service2, event_id2, eventgroup_id2);

        // Create another different SomeIpEvent instance (different event_id)
        let someip_event4 = SomeIpEvent::new(service1, event_id2, eventgroup_id1);

        // Create yet another different SomeIpEvent instance (different eventgroup_id)
        let someip_event5 = SomeIpEvent::new(service1, event_id1, eventgroup_id2);

        assert_eq!(someip_event1, someip_event2, "Identical SomeIpEvents should be equal");
        assert_ne!(
            someip_event1, someip_event3,
            "SomeIpEvents with different services should not be equal"
        );
        assert_ne!(
            someip_event1, someip_event4,
            "SomeIpEvents with different event_ids should not be equal"
        );
        assert_ne!(
            someip_event1, someip_event5,
            "SomeIpEvents with different eventgroup_ids should not be equal"
        );
    }
}
