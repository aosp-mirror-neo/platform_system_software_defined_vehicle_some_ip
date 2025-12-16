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
use std::collections::HashMap;

use sdv_comms::sd::UnitType;

use protobuf::EnumOrUnknown;
use someip_config::broker_params::SdMinorVersionMode;
use someip_config::serde_config::ffi::SomeIpConfig;

use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::Event;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::Interface_type as SomeIpInterfaceType;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::RpcInterface as SomeIpRpcInterface;

use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::simple_event::MessageTranslationMode as VsidlMessageTranslationMode;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::Event_type;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::GroupEvent;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::SimpleEvent;

use vsidl_v1::someip::some_ip_mapping::SomeIpInstanceMapping;
use vsidl_v1::someip::some_ip_mapping::SomeIpServiceMapping as VsidlSomeIpServiceMapping;

use crate::agent::protocol_mapping::SomeIpServiceInstanceInfo;
use crate::mapping_parser::parser::create_someip_service;
use crate::mapping_parser::type_parser::TypeParser;

use crate::agent::protocol_mapping::BrokerServiceUnit;
use crate::agent::protocol_mapping::RpcMethodMappingInfo;
use crate::agent::protocol_mapping::SomeIpEventMappingInfo;
use crate::agent::protocol_mapping::SomeIpEventMappingType;
use crate::agent::protocol_mapping::SomeIpRpcMappingInfo;
use crate::agent::protocol_mapping::SomeIpServiceInstanceMapping;
use crate::agent::protocol_mapping::TypeMapping;
use crate::agent::protocol_mapping::{MessageTranslationMode, SomeIpTypeMapping};

pub(crate) struct SomeIpServiceMappingParser {
    someip_config: SomeIpConfig,
}

impl SomeIpServiceMappingParser {
    pub(crate) fn new(someip_config: SomeIpConfig) -> Self {
        SomeIpServiceMappingParser { someip_config }
    }

    // Process a SOME/IP service mapping
    pub(crate) fn process_someip_service_mapping(
        &self,
        someip_service_mapping: &VsidlSomeIpServiceMapping,
        type_mapping: &mut TypeMapping,
        sd_minor_version_mode: SdMinorVersionMode,
    ) -> Vec<SomeIpServiceInstanceMapping> {
        let mut someip_service_instance_mappings = Vec::new();
        for instance in &someip_service_mapping.instance {
            let Some(someip_service) = create_someip_service(
                someip_service_mapping.someip_service_id,
                instance.instance_id,
                someip_service_mapping.version_major,
                someip_service_mapping.version_minor,
            ) else {
                // Error specifics inside create_someip_service
                error!("Error processing a SOME/IP service instance");
                continue;
            };
            let mut rpc_infos = Vec::new();
            let mut event_infos = Vec::new();
            for instance_impl in &someip_service_mapping.instance_impl {
                let Some(ref implementation) = instance_impl.interface_type else {
                    error!("Error processing a SOME/IP Instance implementation, missing interface_type for service ID {}", someip_service.service_id);
                    continue;
                };
                match implementation {
                    SomeIpInterfaceType::OfferedInterface(rpc) => {
                        let Some(sdv_service_unit_name) =
                            instance.sdv_service_unit_parameterization.get(&rpc.service_unit)
                        else {
                            error!("Error processing a SOME/IP RPC interface implementation, unknown service unit key {} for service ID {}, instance ID {}", &rpc.service_unit, someip_service.service_id, someip_service.instance_id);
                            continue;
                        };
                        if let Some(rpc_mapping) = self.process_someip_rpc(
                            someip_service.service_id,
                            rpc,
                            sdv_service_unit_name,
                            type_mapping,
                        ) {
                            rpc_infos.push(rpc_mapping);
                        }
                    }
                    SomeIpInterfaceType::OfferedEvent(event) => {
                        if let Some(event_mapping) = self.process_someip_event(
                            someip_service.service_id,
                            event,
                            instance,
                            type_mapping,
                        ) {
                            event_infos.push(event_mapping);
                        }
                    }
                    _ => {
                        error!("Error processing a SOME/IP Instance implementation, unknown interface_type for service ID {}", someip_service.service_id);
                    }
                }
            }
            if !rpc_infos.is_empty() || !event_infos.is_empty() {
                someip_service_instance_mappings.push(SomeIpServiceInstanceMapping {
                    someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                        someip_service,
                        sd_minor_version_mode,
                    ),
                    event_infos,
                    rpc_infos,
                });
            } else {
                error!("SOME/IP service with ID {} does not declare any RPC interface or event! Skipping mapping", someip_service.service_id);
            }
        }
        someip_service_instance_mappings
    }

    fn process_someip_rpc(
        &self,
        service_id: u16,
        rpc_interface: &SomeIpRpcInterface,
        service_unit_name: &str,
        type_mapping: &mut TypeMapping,
    ) -> Option<SomeIpRpcMappingInfo> {
        let builder = UnitType::builder();
        let Ok(unit_type) = builder
            .set_sdv_package_name(&rpc_interface.sdv_unit_type.package_name)
            .set_service_bundle_name(&rpc_interface.sdv_unit_type.service_bundle_name)
            .set_unit_type_name(&rpc_interface.sdv_unit_type.type_name)
            .build()
        else {
            error!("Encountered error while building Unit Type object for SOME/IP service mapping, unit type {:?}", rpc_interface.sdv_unit_type);
            return None;
        };

        let mut seen_methods = HashMap::new();
        let mut rpc_method_infos = Vec::new();
        for rpc_method in &rpc_interface.method {
            let Ok(method_id) = u16::try_from(rpc_method.method_id) else {
                error!(
                    "Error processing a SOME/IP RPC interface, method ID {} in service ID {} out of bounds, skipping", rpc_method.method_id, service_id
                );
                continue;
            };
            let prev_value = seen_methods.get(&rpc_method.procedure);
            if prev_value.is_some() {
                error!(
                    "Error processing a SOME/IP RPC interface, SDV procedure name mapping {} in service ID {} already defined, skipping duplicate definition", &rpc_method.procedure, service_id,
                );
                continue;
            }
            let parsed_request =
                TypeParser::parse(&self.someip_config, rpc_method.request_payload_mapping.as_ref());
            let parsed_response = TypeParser::parse(
                &self.someip_config,
                rpc_method.response_payload_mapping.as_ref(),
            );

            match (parsed_request, parsed_response) {
                (Ok(request_definition), Ok(response_definition)) => {
                    seen_methods.insert(
                        rpc_method.procedure.clone(),
                        (
                            SomeIpTypeMapping::new(request_definition),
                            SomeIpTypeMapping::new(response_definition),
                        ),
                    );
                }
                (request_result, response_result) => {
                    if let Err(e) = request_result {
                        error!("Error processing SOME/IP type definition for request parameter in service ID {service_id} and method ID {method_id}, {e:?}");
                    }
                    if let Err(e) = response_result {
                        error!("Error processing SOME/IP type definition for response parameter in service ID {service_id} and method ID {method_id}, {e:?}");
                    }
                    continue;
                }
            }

            rpc_method_infos.push(RpcMethodMappingInfo {
                sdv_method_name: rpc_method.procedure.clone(),
                someip_method_id: method_id,
            });
        }

        type_mapping.rpc_type_mapping.insert(unit_type.clone(), seen_methods);

        SomeIpRpcMappingInfo {
            broker_service_unit: BrokerServiceUnit {
                service_unit_name: service_unit_name.to_string(),
                unit_type,
            },
            rpc_method_infos,
        }
        .into()
    }

    fn process_someip_event(
        &self,
        service_id: u16,
        event: &Event,
        instance: &SomeIpInstanceMapping,
        type_mapping: &mut TypeMapping,
    ) -> Option<SomeIpEventMappingInfo> {
        let Ok(eventgroup_id) = u16::try_from(event.group_id) else {
            error!(
                "Error processing a SOME/IP event, eventgroup ID {} in service ID {} out of bounds, skipping", event.group_id, service_id
            );
            return None;
        };
        let Ok(event_id) = u16::try_from(event.event_id) else {
            error!(
                "Error processing a SOME/IP event, event ID {} in service ID {} out of bounds, skipping", event.event_id, service_id
            );
            return None;
        };

        let Some(ref event_type) = event.event_type else {
            error!("Error processing a SOME/IP event mapping for service ID {service_id} event ID {event_id}, missing event mapping type!");
            return None;
        };

        match event_type {
            Event_type::SimpleEvent(someip_simple_event) => self.process_someip_simple_event(
                service_id,
                eventgroup_id,
                event_id,
                instance,
                someip_simple_event,
                type_mapping,
            ),
            Event_type::GroupEvent(someip_group_event) => self.process_someip_group_event(
                service_id,
                eventgroup_id,
                event_id,
                instance,
                someip_group_event,
            ),
            _ => {
                error!("Error processing a SOME/IP mapping, unknown mapping type!");
                None
            }
        }
    }

    fn get_message_translation_mode(
        mapping_message_translation_mode: Option<EnumOrUnknown<VsidlMessageTranslationMode>>,
    ) -> MessageTranslationMode {
        mapping_message_translation_mode
            .map(|mode| {
                let translation_mode = mode.enum_value_or(VsidlMessageTranslationMode::REFLECTION);
                match translation_mode {
                    VsidlMessageTranslationMode::DYNAMIC_LIBRARY => {
                        MessageTranslationMode::DynamicLibrary
                    }
                    VsidlMessageTranslationMode::INTERPRET_AS_BYTES => {
                        MessageTranslationMode::InterpretAsBytes
                    }
                    _ => MessageTranslationMode::Reflection,
                }
            })
            .unwrap_or(MessageTranslationMode::Reflection)
    }

    fn process_someip_simple_event(
        &self,
        service_id: u16,
        eventgroup_id: u16,
        event_id: u16,
        instance: &SomeIpInstanceMapping,
        someip_simple_event: &SimpleEvent,
        type_mapping: &mut TypeMapping,
    ) -> Option<SomeIpEventMappingInfo> {
        let Some(sdv_service_unit_name) =
            instance.sdv_service_unit_parameterization.get(&someip_simple_event.service_unit)
        else {
            error!("Error processing a SOME/IP event implementation, unknown service unit key {} for service ID {}, event ID {}", &someip_simple_event.service_unit, service_id, event_id);
            return None;
        };
        let builder = UnitType::builder();
        let Ok(unit_type) = builder
            .set_sdv_package_name(&someip_simple_event.sdv_unit_type.package_name)
            .set_service_bundle_name(&someip_simple_event.sdv_unit_type.service_bundle_name)
            .set_unit_type_name(&someip_simple_event.sdv_unit_type.type_name)
            .build()
        else {
            error!("Encountered error while building Unit Type object for SOME/IP service mapping, event ID {}, unit_type {:?}", event_id, someip_simple_event.sdv_unit_type);
            return None;
        };

        let message_translation_mode =
            Self::get_message_translation_mode(someip_simple_event.message_translation_mode);
        match TypeParser::parse(&self.someip_config, someip_simple_event.type_definition.as_ref()) {
            Ok(type_definition) => {
                type_mapping.topic_type_mapping.insert(
                    unit_type.clone(),
                    SomeIpTypeMapping::with_mode(type_definition, message_translation_mode),
                );
                SomeIpEventMappingInfo {
                    event_id,
                    eventgroup_id,
                    event_mapping_type: SomeIpEventMappingType::SIMPLE(Box::new(
                        BrokerServiceUnit {
                            service_unit_name: sdv_service_unit_name.to_string(),
                            unit_type,
                        },
                    )),
                }
                .into()
            }
            Err(e) => {
                error!("Error processing a SOME/IP event implementation, error while parsing type definition for service ID {service_id}, event ID {event_id}, {e:?}");
                None
            }
        }
    }

    fn process_someip_group_event(
        &self,
        service_id: u16,
        eventgroup_id: u16,
        event_id: u16,
        instance: &SomeIpInstanceMapping,
        someip_group_event: &GroupEvent,
    ) -> Option<SomeIpEventMappingInfo> {
        let mut all_parts_parsed = true;
        let mut group_event_infos = Vec::new();
        for part in &someip_group_event.event_part {
            let builder = UnitType::builder();
            let Ok(unit_type) = builder
                .set_sdv_package_name(&part.sdv_unit_type.package_name)
                .set_service_bundle_name(&part.sdv_unit_type.service_bundle_name)
                .set_unit_type_name(&part.sdv_unit_type.type_name)
                .build()
            else {
                error!("Encountered error while building Unit Type object for SOME/IP service mapping, event ID {event_id:?}");
                return None;
            };

            let Some(sdv_service_unit_name) =
                instance.sdv_service_unit_parameterization.get(&part.service_unit)
            else {
                error!("Error processing a SOME/IP event implementation, unknown service unit key {} for service ID {}, event ID {}", &part.service_unit, service_id, event_id);
                all_parts_parsed = false;
                break;
            };
            group_event_infos.push((
                BrokerServiceUnit {
                    service_unit_name: sdv_service_unit_name.to_string(),
                    unit_type,
                },
                part.field_name_mapping.clone(),
            ));
        }

        if all_parts_parsed {
            match TypeParser::parse(
                &self.someip_config,
                someip_group_event.type_definition.as_ref(),
            ) {
                Ok(sometip_type_definition) => SomeIpEventMappingInfo {
                    event_id,
                    eventgroup_id,
                    event_mapping_type: SomeIpEventMappingType::GROUP(
                        sometip_type_definition,
                        group_event_infos,
                    ),
                }
                .into(),
                Err(_) => {
                    error!("Error processing a SOME/IP group event implementation, error processing type definition");
                    None
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use protobuf::EnumOrUnknown;

    use googletest::prelude::*;

    use someip_config::broker_params::SdMinorVersionMode;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};

    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::simple_event::MessageTranslationMode as VsidlMessageTranslationMode;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::Event_type;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::GroupEvent;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::GroupEventPart;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::SimpleEvent;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::rpc_interface::RequestResponseMethod;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::Event;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::Interface_type as SomeIpInterfaceType;
    use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::RpcInterface;

    use vsidl_v1::someip::some_ip_mapping::SomeIpInstanceImplementation;
    use vsidl_v1::someip::some_ip_mapping::SomeIpInstanceMapping;
    use vsidl_v1::someip::some_ip_mapping::SomeIpServiceMapping as VsidlSomeIpServiceMapping;

    use crate::agent::protocol_mapping::*;
    use crate::mapping_parser::someip_parser::SomeIpServiceMappingParser;

    use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;
    use google_sdv_someip::aidl::google::sdv::someip::SomeIpServiceVersion::SomeIpServiceVersion;

    const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::ZeroBytes,
    };

    fn create_someip_service_mapping(
        someip_service_id: u16,
        version_major: u8,
        version_minor: u32,
    ) -> VsidlSomeIpServiceMapping {
        VsidlSomeIpServiceMapping {
            someip_service_id: someip_service_id as u32,
            version_major: version_major as u32,
            version_minor,
            ..Default::default()
        }
    }

    fn add_someip_instance_mapping(
        instance_id: u16,
        sdv_service_unit_parameterization: HashMap<u32, String>,
        someip_service_mapping: &mut VsidlSomeIpServiceMapping,
    ) {
        someip_service_mapping.instance.push(SomeIpInstanceMapping {
            instance_id: instance_id as u32,
            sdv_service_unit_parameterization,
            ..Default::default()
        });
    }

    fn create_rpc_interface(
        unit_type_package: &str,
        unit_type_bundle: &str,
        unit_type_name: &str,
        service_unit: u32,
        method: Vec<RequestResponseMethod>,
    ) -> SomeIpInterfaceType {
        SomeIpInterfaceType::OfferedInterface(RpcInterface {
            sdv_unit_type: Some(vsidl_v1::someip::some_ip_mapping::UnitType {
                package_name: unit_type_package.to_string(),
                service_bundle_name: unit_type_bundle.to_string(),
                type_name: unit_type_name.to_string(),
                ..Default::default()
            })
            .into(),
            service_unit,
            method,
            ..Default::default()
        })
    }

    fn create_rpc_method(procedure: &str, method_id: u16) -> RequestResponseMethod {
        RequestResponseMethod {
            procedure: procedure.to_string(),
            method_id: method_id as u32,
            ..Default::default()
        }
    }

    fn create_simple_event(
        group_id: u16,
        event_id: u16,
        unit_type_package: &str,
        unit_type_bundle: &str,
        unit_type_name: &str,
        service_unit: u32,
    ) -> SomeIpInterfaceType {
        SomeIpInterfaceType::OfferedEvent(Event {
            group_id: group_id as u32,
            event_id: event_id as u32,
            event_type: Event_type::SimpleEvent(SimpleEvent {
                sdv_unit_type: Some(vsidl_v1::someip::some_ip_mapping::UnitType {
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

    fn create_group_event(
        group_id: u16,
        event_id: u16,
        unit_type_package: &str,
        unit_type_bundle: &str,
        unit_type_name: &str,
        group_event_infos: Vec<(u32, HashMap<String, String>)>,
    ) -> SomeIpInterfaceType {
        let mut event_part = Vec::new();
        for (service_unit, field_name_mapping) in group_event_infos {
            event_part.push(GroupEventPart {
                sdv_unit_type: Some(vsidl_v1::someip::some_ip_mapping::UnitType {
                    package_name: unit_type_package.to_string(),
                    service_bundle_name: unit_type_bundle.to_string(),
                    type_name: unit_type_name.to_string(),
                    ..Default::default()
                })
                .into(),
                service_unit,
                field_name_mapping,
                ..Default::default()
            })
        }
        SomeIpInterfaceType::OfferedEvent(Event {
            group_id: group_id as u32,
            event_id: event_id as u32,
            event_type: Event_type::GroupEvent(GroupEvent { event_part, ..Default::default() })
                .into(),
            ..Default::default()
        })
    }

    #[test]
    fn process_someip_service_mapping_detects_someip_event_successfully() {
        let service_id = 2;
        let version_major = 1;
        let version_minor = 2;

        let mut someip_service_mapping =
            create_someip_service_mapping(service_id, version_major, version_minor);

        let instance_id = 1;
        let service_unit_key = 2;
        let service_unit_name = "test_service_unit";
        let mut parameterization = HashMap::new();
        parameterization.insert(service_unit_key, service_unit_name.to_string());

        add_someip_instance_mapping(instance_id, parameterization, &mut someip_service_mapping);

        let group_id: u16 = 1;
        let event_id: u16 = 2;
        let unit_type_package = "sdv.test_package";
        let unit_type_bundle = "TestBundle";
        let unit_type_name = "TestUnitType";

        let event = create_simple_event(
            group_id,
            event_id,
            unit_type_package,
            unit_type_bundle,
            unit_type_name,
            service_unit_key,
        );
        someip_service_mapping.instance_impl.push(SomeIpInstanceImplementation {
            interface_type: Some(event),
            ..Default::default()
        });

        let someip_service_parser = SomeIpServiceMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let someip_service_instance_mappings = someip_service_parser
            .process_someip_service_mapping(
                &someip_service_mapping,
                &mut TypeMapping::default(),
                SdMinorVersionMode::ExactOrAnyMinorVersion,
            );

        let expected_service = SomeIpService {
            service_id,
            instance_id,
            version: SomeIpServiceVersion {
                major: version_major as i8,
                minor: version_minor as i32,
            },
        };

        assert_eq!(someip_service_instance_mappings.len(), 1);

        assert_eq!(
            someip_service_instance_mappings[0]
                .someip_service_instance_info
                .someip_service_instance,
            expected_service
        );
        assert_eq!(someip_service_instance_mappings[0].event_infos.len(), 1);

        let SomeIpEventMappingType::SIMPLE(broker_service_unit) =
            &someip_service_instance_mappings[0].event_infos[0].event_mapping_type
        else {
            panic!("Expected Simple event mapping type");
        };
        let package = broker_service_unit.unit_type.get_sdv_package_name();
        let bundle = broker_service_unit.unit_type.get_service_bundle_name();
        let typename = broker_service_unit.unit_type.get_unit_type_name();
        assert_eq!(broker_service_unit.service_unit_name, service_unit_name.to_string());
        assert_eq!(
            (package, bundle, typename),
            (unit_type_package, "" /* deprecated */, unit_type_name)
        );
    }

    #[test]
    fn process_someip_service_mapping_detects_someip_group_event_successfully() {
        let service_id = 2;
        let version_major = 1;
        let version_minor = 2;
        let mut someip_service_mapping =
            create_someip_service_mapping(service_id, version_major, version_minor);
        let instance_id = 1;
        let service_unit_key1 = 2;
        let service_unit_name1 = "test_service_unit1";

        let service_unit_key2 = 3;
        let service_unit_name2 = "test_service_unit2";
        let mut parameterization = HashMap::new();

        parameterization.insert(service_unit_key1, service_unit_name1.to_string());
        parameterization.insert(service_unit_key2, service_unit_name2.to_string());
        add_someip_instance_mapping(instance_id, parameterization, &mut someip_service_mapping);
        let group_id: u16 = 1;
        let event_id: u16 = 2;
        let unit_type_package = "sdv.test_package";
        let unit_type_bundle = "TestBundle";
        let unit_type_name = "TestUnitType";
        let event = create_group_event(
            group_id,
            event_id,
            unit_type_package,
            unit_type_bundle,
            unit_type_name,
            vec![
                (
                    service_unit_key1,
                    HashMap::from([
                        ("field1_a".to_string(), "field1".to_string()),
                        ("field2_a".to_string(), "field2".to_string()),
                    ]),
                ),
                (
                    service_unit_key2,
                    HashMap::from([
                        ("field1_b".to_string(), "field1".to_string()),
                        ("field2_b".to_string(), "field2".to_string()),
                    ]),
                ),
            ],
        );
        someip_service_mapping.instance_impl.push(SomeIpInstanceImplementation {
            interface_type: Some(event),
            ..Default::default()
        });
        let someip_service_parser = SomeIpServiceMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let someip_service_instance_mappings = someip_service_parser
            .process_someip_service_mapping(
                &someip_service_mapping,
                &mut TypeMapping::default(),
                SdMinorVersionMode::ExactOrAnyMinorVersion,
            );
        let expected_service = SomeIpService {
            service_id,
            instance_id,
            version: SomeIpServiceVersion {
                major: version_major as i8,
                minor: version_minor as i32,
            },
        };
        assert_eq!(someip_service_instance_mappings.len(), 1);
        assert_eq!(
            someip_service_instance_mappings[0]
                .someip_service_instance_info
                .someip_service_instance,
            expected_service
        );
        assert_eq!(someip_service_instance_mappings[0].event_infos.len(), 1);
        let SomeIpEventMappingType::GROUP(_, group_event_infos) =
            &someip_service_instance_mappings[0].event_infos[0].event_mapping_type
        else {
            panic!("Expected Group event mapping type");
        };
        assert_eq!(2, group_event_infos.len());

        for (broker_service_unit, field_name_mapping) in group_event_infos {
            let package = broker_service_unit.unit_type.get_sdv_package_name();
            let bundle = broker_service_unit.unit_type.get_service_bundle_name();
            let typename = broker_service_unit.unit_type.get_unit_type_name();
            assert_eq!(
                (package, bundle, typename),
                (unit_type_package, "" /* deprecated */, unit_type_name)
            );
            let field_names =
                field_name_mapping.values().map(|a| a.to_string()).collect::<Vec<String>>();
            assert_that!(field_names, unordered_elements_are!(eq(&"field1"), eq(&"field2"),))
        }
    }

    #[googletest::test]
    fn process_someip_service_mapping_detects_someip_rpc_successfully() {
        let service_id = 2;
        let version_major = 1;
        let version_minor = 2;

        let mut someip_service_mapping =
            create_someip_service_mapping(service_id, version_major, version_minor);

        let instance_id = 1;
        let service_unit_key = 2;
        let service_unit_name = "test_service_unit";
        let mut parameterization = HashMap::new();
        parameterization.insert(service_unit_key, service_unit_name.to_string());

        add_someip_instance_mapping(instance_id, parameterization, &mut someip_service_mapping);

        let unit_type_package = "sdv.test_package";
        let unit_type_bundle = "TestBundle";
        let unit_type_name = "TestUnitType";
        let (method1_name, method1_id) = ("method1", 20);
        let (method2_name, method2_id) = ("method2", 30);

        let method1 = create_rpc_method(method1_name, method1_id);
        let method2 = create_rpc_method(method2_name, method2_id);

        let rpc = create_rpc_interface(
            unit_type_package,
            unit_type_bundle,
            unit_type_name,
            service_unit_key,
            vec![method1, method2],
        );
        someip_service_mapping
            .instance_impl
            .push(SomeIpInstanceImplementation { interface_type: Some(rpc), ..Default::default() });

        let someip_service_parser = SomeIpServiceMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let someip_service_instance_mappings = someip_service_parser
            .process_someip_service_mapping(
                &someip_service_mapping,
                &mut TypeMapping::default(),
                SdMinorVersionMode::ExactOrAnyMinorVersion,
            );

        let expected_service = SomeIpService {
            service_id,
            instance_id,
            version: SomeIpServiceVersion {
                major: version_major as i8,
                minor: version_minor as i32,
            },
        };

        assert_eq!(someip_service_instance_mappings.len(), 1);

        assert_eq!(
            someip_service_instance_mappings[0]
                .someip_service_instance_info
                .someip_service_instance,
            expected_service
        );
        assert_eq!(someip_service_instance_mappings[0].rpc_infos.len(), 1);

        let unit_type =
            &someip_service_instance_mappings[0].rpc_infos[0].broker_service_unit.unit_type;
        let package = unit_type.get_sdv_package_name();
        let bundle = unit_type.get_service_bundle_name();
        let typename = unit_type.get_unit_type_name();

        assert_eq!(
            someip_service_instance_mappings[0].rpc_infos[0].broker_service_unit.service_unit_name,
            service_unit_name.to_string()
        );
        assert_eq!(
            (package, bundle, typename),
            (unit_type_package, "" /* deprecated */, unit_type_name)
        );
        assert_that!(
            someip_service_instance_mappings[0].rpc_infos[0].rpc_method_infos,
            unordered_elements_are!(
                eq(&RpcMethodMappingInfo {
                    sdv_method_name: method1_name.to_string(),
                    someip_method_id: method1_id
                }),
                eq(&RpcMethodMappingInfo {
                    sdv_method_name: method2_name.to_string(),
                    someip_method_id: method2_id
                }),
            )
        );
    }

    #[test]
    fn test_dynamic_library_mode_parsing_succeeds() {
        let result = SomeIpServiceMappingParser::get_message_translation_mode(Some(
            EnumOrUnknown::new(VsidlMessageTranslationMode::DYNAMIC_LIBRARY),
        ));
        assert_eq!(result, MessageTranslationMode::DynamicLibrary);
    }

    #[test]
    fn test_interpret_as_bytes_mode_parsing_succeeds() {
        let result = SomeIpServiceMappingParser::get_message_translation_mode(Some(
            EnumOrUnknown::new(VsidlMessageTranslationMode::INTERPRET_AS_BYTES),
        ));
        assert_eq!(result, MessageTranslationMode::InterpretAsBytes);
    }

    #[test]
    fn test_reflection_mode_parsing_succeeds() {
        let result = SomeIpServiceMappingParser::get_message_translation_mode(Some(
            EnumOrUnknown::new(VsidlMessageTranslationMode::REFLECTION),
        ));
        assert_eq!(result, MessageTranslationMode::Reflection);
    }

    #[test]
    fn test_unknown_mode_translation_succeeds() {
        let result = SomeIpServiceMappingParser::get_message_translation_mode(Some(
            EnumOrUnknown::from_i32(123),
        ));
        assert_eq!(result, MessageTranslationMode::Reflection);
    }

    #[test]
    fn test_unspecified_mode_translation_succeeds() {
        let result = SomeIpServiceMappingParser::get_message_translation_mode(None);
        assert_eq!(result, MessageTranslationMode::Reflection);
    }
}
