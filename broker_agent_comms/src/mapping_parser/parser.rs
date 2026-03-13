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

use log::error;
use mockall::automock;
use protobuf::EnumOrUnknown;
use someip_config::broker_params::SdMinorVersionMode;
use std::collections::HashMap;
use std::collections::HashSet;

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;
use google_sdv_someip::aidl::google::sdv::someip::SomeIpServiceVersion::SomeIpServiceVersion;
use sdv_comms::sd::UnitType;

use someip_config::serde_config::ffi::SomeIpConfig;

use vsidl_v1::someip::SomeIpMapping;

use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::ServiceUnitId as VsidlServiceUnitId;
use vsidl_v1::someip::some_ip_mapping::some_ip_service_mapping::SdVersionDrivenBehavior;
use vsidl_v1::someip::some_ip_mapping::Mapping_type;
use vsidl_v1::someip::some_ip_mapping::SomeIpServiceMapping;

use crate::agent::protocol_mapping::BrokerMapping;
use crate::agent::protocol_mapping::TypeMapping;
use crate::mapping_parser::sdv_parser::SdvTypeMappingParser;
use crate::mapping_parser::someip_parser::SomeIpServiceMappingParser;

use crate::agent::protocol_mapping::ProtocolMapping;
use crate::agent::protocol_mapping::SdvServiceUnitId;
use crate::agent::protocol_mapping::ServiceFqinInternal;

#[automock]
pub(crate) trait IMappingParser {
    fn parse_mappings(&self, mappings: Vec<SomeIpMapping>) -> BrokerMapping;
}

/// Contains logic that allows to build and cache information about protocol mapping.
pub(crate) struct BrokerMappingParser {
    sdv_type_parser: SdvTypeMappingParser,
    someip_service_parser: SomeIpServiceMappingParser,
    default_sd_minor_version_mode: SdMinorVersionMode,
}

impl IMappingParser for BrokerMappingParser {
    fn parse_mappings(&self, mappings: Vec<SomeIpMapping>) -> BrokerMapping {
        let mut protocol_mapping = ProtocolMapping::default();
        let mut seen_someip_services = HashMap::new();
        let mut seen_sdv_types = HashSet::new();
        let mut type_mapping = TypeMapping::default();
        for mapping in &mappings {
            let Some(ref mapping_type) = mapping.mapping_type else {
                error!("Error processing a SOME/IP mapping, missing mapping type!");
                continue;
            };
            match mapping_type {
                Mapping_type::SomeipServiceMapping(someip_service_mapping) => {
                    let service_sd_minor_version_mode = proto_enum_to_internal_enum(
                        someip_service_mapping.sd_behavior,
                        self.default_sd_minor_version_mode,
                    );
                    if let Err(e) = Self::should_process_someip_service_mapping(
                        someip_service_mapping,
                        service_sd_minor_version_mode,
                        &seen_someip_services,
                    ) {
                        error!("Error processing a SOME/IP mapping, {e}");
                        continue;
                    }
                    seen_someip_services
                        .entry((
                            someip_service_mapping.someip_service_id,
                            someip_service_mapping.version_major,
                        ))
                        .or_insert(HashSet::new())
                        .insert(someip_service_mapping.version_minor);

                    protocol_mapping.someip_service_instances.append(
                        &mut self.someip_service_parser.process_someip_service_mapping(
                            someip_service_mapping,
                            &mut type_mapping,
                            service_sd_minor_version_mode,
                        ),
                    );
                }
                Mapping_type::SdvTypeMapping(sdv_type_mapping) => {
                    let builder = UnitType::builder();
                    let Ok(unit_type) = builder
                        .set_sdv_package_name(&sdv_type_mapping.unit_type.package_name)
                        .set_service_bundle_name(&sdv_type_mapping.unit_type.service_bundle_name)
                        .set_unit_type_name(&sdv_type_mapping.unit_type.type_name)
                        .build()
                    else {
                        error!("Encountered error while building Unit Type object for SOME/IP service mapping, Unit Type {:?}", sdv_type_mapping.unit_type);
                        continue;
                    };

                    if seen_sdv_types.contains(&unit_type) {
                        error!("Error processing a SOME/IP mapping, found duplicate definition for Unit Type {unit_type:?}");
                        continue;
                    }
                    seen_sdv_types.insert(unit_type);
                    self.sdv_type_parser.process_sdv_type_mapping(
                        sdv_type_mapping,
                        &mut protocol_mapping,
                        &mut type_mapping,
                    );
                }
                _ => {
                    error!("Error processing a SOME/IP mapping, unknown mapping type!");
                }
            }
        }
        BrokerMapping { protocol_mapping, type_mapping }
    }
}

impl BrokerMappingParser {
    pub fn new(
        someip_config: SomeIpConfig,
        default_sd_minor_version_mode: SdMinorVersionMode,
    ) -> Self {
        BrokerMappingParser {
            sdv_type_parser: SdvTypeMappingParser::new(someip_config),
            someip_service_parser: SomeIpServiceMappingParser::new(someip_config),
            default_sd_minor_version_mode,
        }
    }

    fn should_process_someip_service_mapping(
        someip_service_mapping: &SomeIpServiceMapping,
        service_sd_minor_version_mode: SdMinorVersionMode,
        seen_someip_services: &HashMap<(u32, u32), HashSet<u32>>,
    ) -> anyhow::Result<()> {
        let service_mapping_key =
            (someip_service_mapping.someip_service_id, someip_service_mapping.version_major);
        if service_sd_minor_version_mode == SdMinorVersionMode::MinimumMinorVersion
            || someip_service_mapping.version_minor == 0xFFFFFFFF
        {
            // We only accept one mapping for services that have their SD behavior set to Minimum Minor Version
            if seen_someip_services.contains_key(&service_mapping_key) {
                anyhow::bail!("Error processing a SOME/IP mapping, found duplicate definition for service with ID {}, major version {}", someip_service_mapping.someip_service_id, someip_service_mapping.version_major);
            }
        } else {
            let is_duplicate = seen_someip_services
                .get(&service_mapping_key)
                .map(|minor_versions| {
                    minor_versions.contains(&someip_service_mapping.version_minor)
                })
                .unwrap_or(false);
            if is_duplicate {
                anyhow::bail!("Error processing a SOME/IP mapping, found duplicate definition for service with ID {}, major version {} and minor version {}", someip_service_mapping.someip_service_id, someip_service_mapping.version_major, someip_service_mapping.version_minor);
            }
        }
        Ok(())
    }
}

pub(crate) fn proto_enum_to_internal_enum(
    enum_or_unknown: Option<EnumOrUnknown<SdVersionDrivenBehavior>>,
    default_sd_minor_version_mode: SdMinorVersionMode,
) -> SdMinorVersionMode {
    if enum_or_unknown.is_none() {
        return default_sd_minor_version_mode;
    }
    match enum_or_unknown.unwrap().enum_value() {
        Ok(enum_value) => match enum_value {
            SdVersionDrivenBehavior::UNKNOWN => default_sd_minor_version_mode,
            SdVersionDrivenBehavior::EXACT_OR_ANY_MINOR_VERSION => {
                SdMinorVersionMode::ExactOrAnyMinorVersion
            }
            SdVersionDrivenBehavior::MINIMUM_MINOR_VERSION => {
                SdMinorVersionMode::MinimumMinorVersion
            }
        },
        Err(_) => default_sd_minor_version_mode,
    }
}

pub(crate) fn create_sdv_service_unit_id(
    service_unit: &VsidlServiceUnitId,
) -> Option<SdvServiceUnitId> {
    let service_fqin = ServiceFqinInternal {
        sdv_vm_name: service_unit.vm_name.clone(),
        sdv_package_name: service_unit.package_name.clone(),
        service_bundle_name: service_unit.service_bundle_name.clone(),
        service_instance_name: service_unit.service_instance_name.clone(),
    };

    SdvServiceUnitId { service_fqin, service_unit_name: service_unit.service_unit_name.clone() }
        .into()
}

pub(crate) fn create_someip_service(
    service: u32,
    instance: u32,
    version_major: u32,
    version_minor: u32,
) -> Option<SomeIpService> {
    let service_id = u16::try_from(service);
    let instance_id = u16::try_from(instance);
    let major = u8::try_from(version_major);

    match (service_id, instance_id, major) {
        (Ok(service_id), Ok(instance_id), Ok(major)) => Some(SomeIpService {
            service_id,
            instance_id,
            version: SomeIpServiceVersion { major: major as i8, minor: version_minor as i32 },
        }),
        _ => {
            if service_id.is_err() {
                error!("SOME/IP Service ID out of range! {service}");
            }
            if instance_id.is_err() {
                error!("SOME/IP Instance ID out of range! {instance}");
            }
            if major.is_err() {
                error!("SOME/IP Major Version out of range! {version_major}");
            }
            Option::None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use vsidl_v1::someip::some_ip_mapping::{
        sdv_type_mapping::{
            topic::{SimpleTopicMapping, Topic_mapping},
            Interface_type, SdvServiceUnitMapping, ServiceUnitId, Topic,
        },
        Mapping_type, SdvTypeMapping, SomeIpServiceMapping, UnitType as VsidlUnitType,
    };

    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::Event_type;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::SimpleEvent;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::Event;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::Interface_type as SomeIpInterfaceType;
    use vsidl_v1::someip::some_ip_mapping::{SomeIpInstanceImplementation, SomeIpInstanceMapping};

    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};

    const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::ZeroBytes,
    };

    fn create_simple_event(
        group_id: u32,
        event_id: u32,
        unit_type_package: &str,
        unit_type_bundle: &str,
        unit_type_name: &str,
        service_unit: u32,
    ) -> SomeIpInterfaceType {
        SomeIpInterfaceType::OfferedEvent(Event {
            group_id,
            event_id,
            event_type: Event_type::SimpleEvent(SimpleEvent {
                sdv_unit_type: Some(VsidlUnitType {
                    package_name: unit_type_package.to_string(),
                    service_bundle_name: unit_type_bundle.to_string(),
                    type_name: unit_type_name.to_string(),
                    ..Default::default()
                })
                .into(),
                service_unit,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    fn create_test_vsidl_unit_type(pkg: &str, bundle: &str, type_name: &str) -> VsidlUnitType {
        VsidlUnitType {
            package_name: pkg.to_string(),
            service_bundle_name: bundle.to_string(),
            type_name: type_name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_create_someip_service_valid() {
        let service = create_someip_service(1, 2, 3, 4);
        assert!(service.is_some());
        let service = service.unwrap();
        assert_eq!(service.service_id, 1);
        assert_eq!(service.instance_id, 2);
        assert_eq!(service.version.major, 3);
        assert_eq!(service.version.minor, 4);
    }

    #[test]
    fn test_create_someip_service_invalid_service_id() {
        let service = create_someip_service(u16::MAX as u32 + 1, 2, 3, 4);
        assert!(service.is_none());
    }

    #[test]
    fn test_create_someip_service_invalid_instance_id() {
        let service = create_someip_service(1, u16::MAX as u32 + 1, 3, 4);
        assert!(service.is_none());
    }

    #[test]
    fn test_create_someip_service_invalid_major_version() {
        let service = create_someip_service(1, 2, u8::MAX as u32 + 1, 4);
        assert!(service.is_none());
    }

    #[test]
    fn test_create_sdv_service_unit_id_valid() {
        let vsidl_suid = VsidlServiceUnitId {
            vm_name: Some("vm1".to_string()),
            package_name: "com.example".to_string(),
            service_bundle_name: "MyBundle".to_string(),
            service_instance_name: Some("instance1".to_string()),
            service_unit_name: "Unit1".to_string(),
            ..Default::default()
        };

        let info = create_sdv_service_unit_id(&vsidl_suid);
        assert!(info.is_some());
        let info = info.unwrap();

        assert_eq!(info.service_fqin.sdv_vm_name, Some("vm1".to_string()));
        assert_eq!(info.service_fqin.sdv_package_name, "com.example");
        assert_eq!(info.service_fqin.service_bundle_name, "MyBundle");
        assert_eq!(info.service_fqin.service_instance_name, Some("instance1".to_string()));
        assert_eq!(info.service_unit_name, "Unit1");
    }

    #[test]
    fn test_broker_mapping_parser_empty_mappings() {
        let parser = BrokerMappingParser::new(
            DEFAULT_SOMEIP_CONFIG,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let mappings = Vec::new();
        let broker_mapping = parser.parse_mappings(mappings);

        assert!(broker_mapping.protocol_mapping.sdv_service_units.is_empty());
        assert!(broker_mapping.protocol_mapping.sdv_multipub_topics.is_empty());
        assert!(broker_mapping.protocol_mapping.someip_service_instances.is_empty());
        assert!(broker_mapping.type_mapping.topic_type_mapping.is_empty());
        assert!(broker_mapping.type_mapping.rpc_type_mapping.is_empty());
    }

    #[test]
    fn test_broker_mapping_parser_someip_service_mapping() {
        let parser = BrokerMappingParser::new(
            DEFAULT_SOMEIP_CONFIG,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let service_unit_key = 3;
        let event =
            create_simple_event(1, 2, "com.example", "TestBundle", "TestType", service_unit_key);
        let someip_mapping_proto = SomeIpServiceMapping {
            someip_service_id: 1,
            version_major: 2,
            version_minor: 3,
            instance: vec![SomeIpInstanceMapping {
                instance_id: 1,
                sdv_service_unit_parameterization: HashMap::from([(
                    service_unit_key,
                    "service-unit".to_string(),
                )]),
                ..Default::default()
            }],
            instance_impl: vec![SomeIpInstanceImplementation {
                interface_type: Some(event),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mapping = SomeIpMapping {
            mapping_type: Some(Mapping_type::SomeipServiceMapping(someip_mapping_proto)),
            ..Default::default()
        };
        let broker_mapping = parser.parse_mappings(vec![mapping]);
        // This test focuses on dispatch and aggregation.
        // If SomeIpServiceMappingParser adds an instance, it should be here.
        assert!(!broker_mapping.protocol_mapping.someip_service_instances.is_empty());
    }

    #[test]
    fn test_broker_mapping_parser_sdv_type_mapping() {
        let parser = BrokerMappingParser::new(
            DEFAULT_SOMEIP_CONFIG,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let sdv_mapping_proto = SdvTypeMapping {
            unit_type: Some(create_test_vsidl_unit_type("com.example", "TestBundle", "TestType"))
                .into(),
            someip_service_id: 10,
            version_major: 1,
            version_minor: 0,
            service_unit_mapping: vec![SdvServiceUnitMapping {
                service_unit_id: Some(ServiceUnitId {
                    vm_name: "vm".to_string().into(),
                    package_name: "com.example".to_string(),
                    service_bundle_name: "TestBundle".to_string(),
                    service_instance_name: "instance1".to_string().into(),
                    service_unit_name: "test-unit".to_string(),
                    ..Default::default()
                })
                .into(),
                instance_id: 1,
                ..Default::default()
            }],
            interface_type: Some(Interface_type::OfferedTopic(Topic {
                topic_mapping: Some(Topic_mapping::SimpleMapping(SimpleTopicMapping {
                    group_id: 1,
                    event_id: 1,
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let mapping = SomeIpMapping {
            mapping_type: Some(Mapping_type::SdvTypeMapping(sdv_mapping_proto)),
            ..Default::default()
        };
        let broker_mapping = parser.parse_mappings(vec![mapping]);
        // SdvTypeMappingParser populates sdv_service_units or sdv_multipub_topics.
        assert!(!broker_mapping.protocol_mapping.sdv_service_units.is_empty());
    }

    #[test]
    fn test_broker_mapping_parser_duplicate_someip_service_mapping_is_skipped() {
        let parser = BrokerMappingParser::new(
            DEFAULT_SOMEIP_CONFIG,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let service_unit_key = 3;
        let event =
            create_simple_event(1, 2, "com.example", "TestBundle", "TestType", service_unit_key);
        let someip_mapping_proto = SomeIpServiceMapping {
            someip_service_id: 1,
            version_major: 2,
            version_minor: 3,
            instance: vec![SomeIpInstanceMapping {
                instance_id: 1,
                sdv_service_unit_parameterization: HashMap::from([(
                    service_unit_key,
                    "service-unit".to_string(),
                )]),
                ..Default::default()
            }],
            instance_impl: vec![SomeIpInstanceImplementation {
                interface_type: Some(event),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mapping1 = SomeIpMapping {
            mapping_type: Some(Mapping_type::SomeipServiceMapping(someip_mapping_proto.clone())),
            ..Default::default()
        };
        let mapping2 = SomeIpMapping {
            // Duplicate
            mapping_type: Some(Mapping_type::SomeipServiceMapping(someip_mapping_proto)),
            ..Default::default()
        };

        let broker_mapping = parser.parse_mappings(vec![mapping1, mapping2]);
        // Expecting only one instance due to duplicate skipping
        assert_eq!(broker_mapping.protocol_mapping.someip_service_instances.len(), 1);
    }

    #[test]
    fn test_broker_mapping_parser_duplicate_sdv_type_mapping_is_skipped() {
        let parser = BrokerMappingParser::new(
            DEFAULT_SOMEIP_CONFIG,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let sdv_mapping_proto = SdvTypeMapping {
            unit_type: Some(create_test_vsidl_unit_type("com.example", "TestBundle", "TestType"))
                .into(),
            someip_service_id: 10,
            version_major: 1,
            version_minor: 0,
            service_unit_mapping: vec![SdvServiceUnitMapping {
                service_unit_id: Some(ServiceUnitId {
                    vm_name: "vm".to_string().into(),
                    package_name: "com.example".to_string(),
                    service_bundle_name: "TestBundle".to_string(),
                    service_instance_name: "instance1".to_string().into(),
                    service_unit_name: "test-unit".to_string(),
                    ..Default::default()
                })
                .into(),
                instance_id: 1,
                ..Default::default()
            }],
            interface_type: Some(Interface_type::OfferedTopic(Topic {
                topic_mapping: Some(Topic_mapping::SimpleMapping(SimpleTopicMapping {
                    group_id: 1,
                    event_id: 1,
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let mapping1 = SomeIpMapping {
            mapping_type: Some(Mapping_type::SdvTypeMapping(sdv_mapping_proto.clone())),
            ..Default::default()
        };
        let mapping2 = SomeIpMapping {
            // Duplicate
            mapping_type: Some(Mapping_type::SdvTypeMapping(sdv_mapping_proto)),
            ..Default::default()
        };
        let broker_mapping = parser.parse_mappings(vec![mapping1, mapping2]);
        // Expecting only one sdv_service_unit due to duplicate skipping
        assert_eq!(broker_mapping.protocol_mapping.sdv_service_units.len(), 1);
    }

    #[test]
    fn test_broker_mapping_parser_mapping_type_none() {
        let parser = BrokerMappingParser::new(
            DEFAULT_SOMEIP_CONFIG,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let mapping = SomeIpMapping { mapping_type: None, ..Default::default() };
        let broker_mapping = parser.parse_mappings(vec![mapping]);
        assert!(broker_mapping.protocol_mapping.sdv_service_units.is_empty());
        assert!(broker_mapping.protocol_mapping.someip_service_instances.is_empty());
    }

    #[test]
    fn test_broker_mapping_parser_sdv_type_mapping_invalid_unit_type_build() {
        let parser = BrokerMappingParser::new(
            DEFAULT_SOMEIP_CONFIG,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let sdv_mapping_proto = SdvTypeMapping {
            unit_type: Some(create_test_vsidl_unit_type("", "", "")).into(),
            someip_service_id: 10,
            version_major: 1,
            version_minor: 0,
            service_unit_mapping: vec![SdvServiceUnitMapping {
                service_unit_id: Some(ServiceUnitId {
                    vm_name: "vm".to_string().into(),
                    package_name: "com.example".to_string(),
                    service_bundle_name: "TestBundle".to_string(),
                    service_instance_name: "instance1".to_string().into(),
                    service_unit_name: "test-unit".to_string(),
                    ..Default::default()
                })
                .into(),
                instance_id: 1,
                ..Default::default()
            }],
            interface_type: Some(Interface_type::OfferedTopic(Topic {
                topic_mapping: Some(Topic_mapping::SimpleMapping(SimpleTopicMapping {
                    group_id: 1,
                    event_id: 1,
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let mapping = SomeIpMapping {
            mapping_type: Some(Mapping_type::SdvTypeMapping(sdv_mapping_proto)),
            ..Default::default()
        };
        let broker_mapping = parser.parse_mappings(vec![mapping]);
        assert!(broker_mapping.protocol_mapping.sdv_service_units.is_empty());
    }

    fn create_test_someip_service_mapping(
        service_id: u32,
        major: u32,
        minor: u32,
    ) -> SomeIpServiceMapping {
        SomeIpServiceMapping {
            someip_service_id: service_id,
            version_major: major,
            version_minor: minor,
            ..Default::default()
        }
    }

    #[test]
    fn test_should_process_minimum_minor_version_new_service() {
        let mapping = create_test_someip_service_mapping(1, 1, 10);
        let seen_services = HashMap::new();
        assert!(BrokerMappingParser::should_process_someip_service_mapping(
            &mapping,
            SdMinorVersionMode::MinimumMinorVersion,
            &seen_services
        )
        .is_ok());
    }

    #[test]
    fn test_should_not_process_minimum_minor_version_duplicate_service() {
        let mapping = create_test_someip_service_mapping(1, 1, 10);
        let mut seen_services = HashMap::new();
        seen_services.entry((1, 1)).or_insert_with(HashSet::new).insert(5);
        assert!(BrokerMappingParser::should_process_someip_service_mapping(
            &mapping,
            SdMinorVersionMode::MinimumMinorVersion,
            &seen_services
        )
        .is_err());
    }

    #[test]
    fn test_should_process_any_minor_version_new_service() {
        // 0xFFFFFFFF is the "any" minor version
        let mapping = create_test_someip_service_mapping(1, 1, 0xFFFFFFFF);
        let seen_services = HashMap::new();
        assert!(BrokerMappingParser::should_process_someip_service_mapping(
            &mapping,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
            &seen_services
        )
        .is_ok());
    }

    #[test]
    fn test_should_not_process_any_minor_version_duplicate_service() {
        let mapping = create_test_someip_service_mapping(1, 1, 0xFFFFFFFF);
        let mut seen_services = HashMap::new();
        seen_services.entry((1, 1)).or_insert_with(HashSet::new).insert(5);
        assert!(BrokerMappingParser::should_process_someip_service_mapping(
            &mapping,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
            &seen_services
        )
        .is_err());
    }

    #[test]
    fn test_should_process_exact_minor_version_new_service() {
        let mapping = create_test_someip_service_mapping(1, 1, 10);
        let seen_services = HashMap::new();
        assert!(BrokerMappingParser::should_process_someip_service_mapping(
            &mapping,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
            &seen_services
        )
        .is_ok());
    }

    #[test]
    fn test_should_process_exact_minor_version_new_minor_for_existing_service() {
        let mapping = create_test_someip_service_mapping(1, 1, 10);
        let mut seen_services = HashMap::new();
        seen_services.entry((1, 1)).or_insert_with(HashSet::new).insert(5);
        assert!(BrokerMappingParser::should_process_someip_service_mapping(
            &mapping,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
            &seen_services
        )
        .is_ok());
    }

    #[test]
    fn test_should_not_process_exact_minor_version_duplicate_service_and_minor() {
        let mapping = create_test_someip_service_mapping(1, 1, 10);
        let mut seen_services = HashMap::new();
        seen_services.entry((1, 1)).or_insert_with(HashSet::new).insert(10);
        assert!(BrokerMappingParser::should_process_someip_service_mapping(
            &mapping,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
            &seen_services
        )
        .is_err());
    }
}
