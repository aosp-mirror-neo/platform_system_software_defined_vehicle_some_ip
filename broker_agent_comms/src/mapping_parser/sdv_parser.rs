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

use someip_config::serde_config::ffi::SomeIpConfig;

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;

use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::topic::Topic_mapping;
use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::Interface_type as SdvInterfaceType;
use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::RpcInterface as SdvRpcInterface;
use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::SomeIpFFMethod;
use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::Topic;
use vsidl_v1::someip::some_ip_mapping::SdvTypeMapping;

use crate::agent::protocol_mapping::SdvMultipubMapping;
use crate::mapping_parser::parser::{create_sdv_service_unit_id, create_someip_service};
use crate::mapping_parser::type_parser::TypeParser;

use crate::agent::protocol_mapping::{
    ProtocolMapping, RpcMethodMappingInfo, SdvIdentifier, SdvServiceUnitInterface,
    SdvServiceUnitMapping, SdvTopicMappingInfo, SomeIpMethod, SomeIpTypeMapping, TypeMapping,
};

pub(crate) struct SdvTypeMappingParser {
    someip_config: SomeIpConfig,
}

impl SdvTypeMappingParser {
    pub(crate) fn new(someip_config: SomeIpConfig) -> Self {
        SdvTypeMappingParser { someip_config }
    }

    /// populates the given `type_mapping` and `protocol_mapping` with the information obtained by processing the provided `sdv_type_mapping`
    pub(crate) fn process_sdv_type_mapping(
        &self,
        sdv_type_mapping: &SdvTypeMapping,
        protocol_mapping: &mut ProtocolMapping,
        type_mapping: &mut TypeMapping,
    ) {
        let builder = UnitType::builder();
        let Ok(unit_type) = builder
            .set_sdv_package_name(&sdv_type_mapping.unit_type.package_name)
            .set_service_bundle_name(&sdv_type_mapping.unit_type.service_bundle_name)
            .set_unit_type_name(&sdv_type_mapping.unit_type.type_name)
            .build()
        else {
            error!("Encountered error while building Unit Type object for SOME/IP service mapping, Unit Type {:?}", sdv_type_mapping.unit_type);
            return;
        };

        let Some(someip_service_instance_info) = create_someip_service(
            sdv_type_mapping.someip_service_id,
            0,
            sdv_type_mapping.version_major,
            sdv_type_mapping.version_minor,
        ) else {
            // Error specifics inside create_someip_service
            error!("Error processing a SOME/IP service instance");
            return;
        };

        if let protobuf::MessageField(Some(ref _service_oriented_mapping)) =
            sdv_type_mapping.service_oriented_mapping
        {
            // TODO: b/487204223 - implement SOA logic in SOME/IP broker
        } else {
            self.process_service_unit_mapping(
                &unit_type,
                sdv_type_mapping,
                someip_service_instance_info,
                type_mapping,
                protocol_mapping,
            );
        }
    }

    fn process_service_unit_mapping(
        &self,
        unit_type: &UnitType,
        sdv_type_mapping: &SdvTypeMapping,
        mut someip_service_instance_info: SomeIpService,
        type_mapping: &mut TypeMapping,
        protocol_mapping: &mut ProtocolMapping,
    ) {
        let Some(ref implementation) = sdv_type_mapping.interface_type else {
            error!(
                "Error processing a SDV Type mapping (Type: Service Unit), missing interface_type for unit type {unit_type:?}"
            );
            return;
        };
        let processed_implementation = match implementation {
            SdvInterfaceType::OfferedInterface(rpc_interface) => {
                self.process_sdv_rpc(unit_type, rpc_interface, type_mapping).into()
            }
            SdvInterfaceType::OfferedTopic(topic) => self.process_sdv_topic(
                unit_type,
                topic,
                someip_service_instance_info,
                protocol_mapping,
                type_mapping,
            ),
            _ => {
                error!("Error processing a SDV service unit interface, unknown interface_type for unit type {unit_type:?}");
                None
            }
        };
        let Some(service_unit_interface) = processed_implementation else {
            return;
        };

        let mut sdv_service_unit_mappings = Vec::new();
        for service_unit_mapping in &sdv_type_mapping.service_unit_mapping {
            let Ok(instance_id) = u16::try_from(service_unit_mapping.instance_id) else {
                error!(
                    "Error processing a SOME/IP service instance, instance ID out of range! {}",
                    service_unit_mapping.instance_id
                );
                continue;
            };
            someip_service_instance_info.instance_id = instance_id;

            let Some(service_unit_id) =
                create_sdv_service_unit_id(&service_unit_mapping.service_unit_id)
            else {
                continue;
            };

            if let SdvServiceUnitInterface::TOPIC(SdvTopicMappingInfo::FireAndForgetMapping {
                ff_mapping: _,
            }) = service_unit_interface
            {
                let ff_mapping = Self::get_fire_and_forget_mappings(
                    &service_unit_mapping.fire_and_forget_method,
                );
                sdv_service_unit_mappings.push(SdvServiceUnitMapping {
                    sdv_identifier: SdvIdentifier::ServiceUnitInfo { service_unit_id },
                    unit_type: unit_type.clone(),
                    someip_service_instance_info,
                    service_unit_interface: SdvServiceUnitInterface::TOPIC(
                        SdvTopicMappingInfo::FireAndForgetMapping { ff_mapping },
                    ),
                });
            } else {
                sdv_service_unit_mappings.push(SdvServiceUnitMapping {
                    sdv_identifier: SdvIdentifier::ServiceUnitInfo { service_unit_id },
                    unit_type: unit_type.clone(),
                    someip_service_instance_info,
                    service_unit_interface: service_unit_interface.clone(),
                });
            }
        }
        protocol_mapping.sdv_service_units.append(&mut sdv_service_unit_mappings);
    }

    fn process_sdv_rpc(
        &self,
        unit_type: &UnitType,
        rpc_interface: &SdvRpcInterface,
        type_mapping: &mut TypeMapping,
    ) -> SdvServiceUnitInterface {
        let mut method_mapping_info = Vec::new();
        let mut seen_methods = HashMap::new();
        for method in &rpc_interface.method {
            let Ok(someip_method_id) = u16::try_from(method.method_id) else {
                error!(
                    "Error processing a SDV RPC mapping, method ID {} for unit type {:?} out of bounds, skipping", method.method_id, unit_type
                );
                continue;
            };
            let prev_value = seen_methods.get(&method.procedure);
            if prev_value.is_some() {
                error!(
                    "Error processing a SDV RPC interface, SDV procedure name {} for unit type {:?} already defined, skipping duplicate definition", &method.procedure, unit_type,
                );
                continue;
            }
            let parsed_request =
                TypeParser::parse(&self.someip_config, method.request_payload_mapping.as_ref());
            let parsed_response =
                TypeParser::parse(&self.someip_config, method.response_payload_mapping.as_ref());
            match (parsed_request, parsed_response) {
                (Ok(request_definition), Ok(response_definition)) => {
                    seen_methods.insert(
                        method.procedure.clone(),
                        (
                            SomeIpTypeMapping::new(request_definition),
                            SomeIpTypeMapping::new(response_definition),
                        ),
                    );
                }
                (request_result, response_result) => {
                    if let Err(e) = request_result {
                        error!("Error processing SOME/IP type definition for request parameter in unit type {unit_type:?} and procedure {}, {e:?}", method.procedure);
                    }
                    if let Err(e) = response_result {
                        error!("Error processing SOME/IP type definition for response parameter in unit type {unit_type:?} and procedure {}, {e:?}", method.procedure);
                    }
                    continue;
                }
            }
            method_mapping_info.push(RpcMethodMappingInfo {
                sdv_method_name: method.procedure.clone(),
                someip_method_id,
            });
        }
        type_mapping.rpc_type_mapping.insert(unit_type.clone(), seen_methods);
        SdvServiceUnitInterface::RPC(method_mapping_info)
    }

    /// Processes the given SDV topic and returns the processed definition in case the input corresponds to a correctly specified non-multipub topic, else returns None.
    /// Additionally, may populate the given `protocol_mapping` in case the topic corresponds to a multipub topic
    fn process_sdv_topic(
        &self,
        unit_type: &UnitType,
        topic: &Topic,
        mut multipub_service_instance_info: SomeIpService,
        protocol_mapping: &mut ProtocolMapping,
        type_mapping: &mut TypeMapping,
    ) -> Option<SdvServiceUnitInterface> {
        let Some(ref topic_mapping) = topic.topic_mapping else {
            error!("Error processing a SDV topic mapping, missing topic_mapping!");
            return None;
        };
        match TypeParser::parse(&self.someip_config, topic.type_definition.as_ref()) {
            Ok(type_definition) => {
                type_mapping
                    .topic_type_mapping
                    .insert(unit_type.clone(), SomeIpTypeMapping::new(type_definition));
            }
            Err(e) => {
                error!("Error processing a SDV topic mapping while parsing type definition for unit type {unit_type:?}, {e:?}");
                return None;
            }
        };

        match topic_mapping {
            Topic_mapping::SimpleMapping(simple_mapping) => {
                let Ok(someip_eventgroup_id) = u16::try_from(simple_mapping.group_id) else {
                    error!(
                        "Error processing a SDV topic simple mapping, eventgroup ID {} for unit type {:?} out of bounds, skipping", simple_mapping.group_id, unit_type
                    );
                    return None;
                };
                let Ok(someip_event_id) = u16::try_from(simple_mapping.event_id) else {
                    error!(
                        "Error processing a SDV topic simple mapping, event ID {} for unit type {:?} out of bounds, skipping", simple_mapping.event_id, unit_type
                    );
                    return None;
                };

                match topic.multipub_topic_info.as_ref() {
                    Some(multipub_topic_info) => {
                        let multipub_service_instance_id = multipub_topic_info.someip_instance_id;
                        let Ok(instance_id) = u16::try_from(multipub_service_instance_id) else {
                            error!("Error processing a multipub topic, instance ID out of range! {multipub_service_instance_id}");
                            return None;
                        };
                        multipub_service_instance_info.instance_id = instance_id;
                        protocol_mapping.sdv_multipub_topics.push(SdvMultipubMapping {
                            unit_type: unit_type.clone(),
                            someip_service_instance_info: multipub_service_instance_info,
                            someip_event_id,
                            someip_eventgroup_id,
                        });
                        None
                    }
                    None => {
                        // return the service unit info to be handled by caller
                        SdvServiceUnitInterface::TOPIC(SdvTopicMappingInfo::SimpleMapping {
                            someip_eventgroup_id,
                            someip_event_id,
                        })
                        .into()
                    }
                }
            }
            Topic_mapping::SomeipFfMethods(use_ff) => {
                if topic.multipub_topic_info.is_some() {
                    error!("F&F mapping detected for multipub topic, which is not supported");
                    return None;
                }
                if !use_ff {
                    return None;
                }
                SdvServiceUnitInterface::TOPIC(SdvTopicMappingInfo::FireAndForgetMapping {
                    ff_mapping: Vec::new(),
                })
                .into()
            }
            _ => {
                error!("Error processing a SOME/IP mapping, unknown mapping type!");
                None
            }
        }
    }

    fn get_fire_and_forget_mappings(ff_methods: &Vec<SomeIpFFMethod>) -> Vec<SomeIpMethod> {
        let mut ff_mappings = Vec::new();
        for ff_method in ff_methods {
            let Some(someip_service) = create_someip_service(
                ff_method.service_id,
                ff_method.instance_id,
                ff_method.version_major,
                ff_method.version_minor,
            ) else {
                // Error specifics inside create_someip_service
                error!("Error processing a SOME/IP service instance");
                continue;
            };
            let Ok(someip_method_id) = u16::try_from(ff_method.method_id) else {
                error!(
                    "Error processing a SDV topic F&F mapping, method ID {} in service ID {} out of bounds, skipping", ff_method.method_id, ff_method.service_id
                );
                continue;
            };
            ff_mappings.push(SomeIpMethod { someip_service, someip_method_id })
        }
        ff_mappings
    }
}

#[cfg(test)]
mod tests {

    use googletest::prelude::*;

    use vsidl_v1::someip::some_ip_mapping::SdvTypeMapping;
    use vsidl_v1::someip::some_ip_mapping::UnitType as VsidlUnitType;

    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::rpc_interface::Method;
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::topic::Topic_mapping;
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::topic::{
        MultipubTopicInfo, SimpleTopicMapping,
    };
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::Interface_type as SdvInterfaceType;
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::RpcInterface as VsidlRpcInterface;
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::SdvServiceUnitMapping as VsidlSdvServiceUnitMapping;
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::ServiceUnitId as VsidlServiceUnitId;
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::SomeIpFFMethod as VsidlSomeIpFFMethod;
    use vsidl_v1::someip::some_ip_mapping::sdv_type_mapping::Topic;

    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};

    use sdv_comms::sd::UnitType;

    use crate::agent::protocol_mapping::*;
    use crate::mapping_parser::sdv_parser::SdvTypeMappingParser;

    use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;
    use google_sdv_someip::aidl::google::sdv::someip::SomeIpServiceVersion::SomeIpServiceVersion;

    const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::ZeroBytes,
    };

    fn create_sdv_type_mapping(
        unit_type: &UnitType,
        someip_service: &SomeIpService,
        interface_type: Option<SdvInterfaceType>,
    ) -> SdvTypeMapping {
        SdvTypeMapping {
            unit_type: Some(VsidlUnitType {
                package_name: unit_type.get_sdv_package_name().to_string(),
                service_bundle_name: unit_type.get_service_bundle_name().to_string(),
                type_name: unit_type.get_unit_type_name().to_string(),
                ..Default::default()
            })
            .into(),
            someip_service_id: someip_service.service_id as u32,
            version_major: someip_service.version.major as u32,
            version_minor: someip_service.version.minor as u32,
            interface_type,
            ..Default::default()
        }
    }

    fn add_implementation_to_sdv_type_mapping(
        mapping: &mut SdvTypeMapping,
        instance_id: u16,
        sdv_service_unit_id: &SdvServiceUnitId,
    ) {
        mapping.service_unit_mapping.push(VsidlSdvServiceUnitMapping {
            service_unit_id: Some(VsidlServiceUnitId {
                vm_name: sdv_service_unit_id.service_fqin.sdv_vm_name.clone().unwrap().into(),
                package_name: sdv_service_unit_id.service_fqin.sdv_package_name.clone(),
                service_bundle_name: sdv_service_unit_id.service_fqin.service_bundle_name.clone(),
                service_instance_name: sdv_service_unit_id
                    .service_fqin
                    .service_instance_name
                    .clone()
                    .unwrap()
                    .into(),
                service_unit_name: sdv_service_unit_id.service_unit_name.clone(),
                ..Default::default()
            })
            .into(),
            instance_id: instance_id as u32,
            fire_and_forget_method: Vec::new(),
            ..Default::default()
        });
    }

    #[test]
    fn process_sdv_type_mapping_detects_sdv_topic_unit_successfully() {
        let builder = UnitType::builder();
        let unit_type = builder
            .set_sdv_package_name("com.test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitType")
            .build()
            .expect("Failed to build Unit type!");
        let someip_service_instance = SomeIpService {
            service_id: 2,
            instance_id: 1,
            version: SomeIpServiceVersion { major: 1, minor: 2 },
        };
        let group_id: u16 = 1;
        let event_id: u16 = 2;

        let sdv_interface_type = SdvInterfaceType::OfferedTopic(Topic {
            topic_mapping: Some(Topic_mapping::SimpleMapping(SimpleTopicMapping {
                group_id: group_id as u32,
                event_id: event_id as u32,
                ..Default::default()
            })),
            ..Default::default()
        })
        .into();

        let mut sdv_type_mapping =
            create_sdv_type_mapping(&unit_type, &someip_service_instance, sdv_interface_type);
        let service_fqin = ServiceFqinInternal {
            sdv_vm_name: String::from("test_vm").into(),
            sdv_package_name: String::from("com.test.package"),
            service_bundle_name: String::from("TestBundle"),
            service_instance_name: String::from("test_instance").into(),
        };

        let sdv_service_unit_id =
            SdvServiceUnitId { service_fqin, service_unit_name: "test_service_unit".to_string() };

        add_implementation_to_sdv_type_mapping(
            &mut sdv_type_mapping,
            someip_service_instance.instance_id,
            &sdv_service_unit_id,
        );

        let sdv_type_parser = SdvTypeMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let mut protocol_mapping = ProtocolMapping::default();
        sdv_type_parser.process_sdv_type_mapping(
            &sdv_type_mapping,
            &mut protocol_mapping,
            &mut TypeMapping::default(),
        );

        let sdv_service_unit_mappings = protocol_mapping.sdv_service_units;

        assert_eq!(sdv_service_unit_mappings.len(), 1);

        let SdvIdentifier::ServiceUnitInfo { ref service_unit_id } =
            sdv_service_unit_mappings[0].sdv_identifier
        else {
            panic!("Unexpected identifier type")
        };
        assert_eq!(*service_unit_id, sdv_service_unit_id);
        assert_eq!(sdv_service_unit_mappings[0].unit_type, unit_type);

        assert_eq!(
            sdv_service_unit_mappings[0].someip_service_instance_info,
            someip_service_instance
        );
        assert_eq!(
            sdv_service_unit_mappings[0].service_unit_interface,
            SdvServiceUnitInterface::TOPIC(SdvTopicMappingInfo::SimpleMapping {
                someip_event_id: event_id,
                someip_eventgroup_id: group_id
            })
        );
    }

    #[googletest::test]
    fn process_sdv_type_mapping_detects_sdv_rpc_unit_successfully() {
        let builder = UnitType::builder();
        let unit_type = builder
            .set_sdv_package_name("com.test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitType")
            .build()
            .expect("Failed to build Unit type!");

        let someip_service_instance = SomeIpService {
            service_id: 2,
            instance_id: 1,
            version: SomeIpServiceVersion { major: 1, minor: 2 },
        };

        let rpc_method1 =
            Method { procedure: "FirstProc".to_string(), method_id: 5, ..Default::default() };
        let rpc_method2 =
            Method { procedure: "SecondProc".to_string(), method_id: 4, ..Default::default() };

        let sdv_interface_type = SdvInterfaceType::OfferedInterface(VsidlRpcInterface {
            method: vec![rpc_method1.clone(), rpc_method2.clone()],
            ..Default::default()
        })
        .into();

        let mut sdv_type_mapping =
            create_sdv_type_mapping(&unit_type, &someip_service_instance, sdv_interface_type);
        let service_fqin = ServiceFqinInternal {
            sdv_vm_name: String::from("test_vm").into(),
            sdv_package_name: String::from("com.test.package"),
            service_bundle_name: String::from("TestBundle"),
            service_instance_name: String::from("test_instance").into(),
        };
        let sdv_service_unit_id =
            SdvServiceUnitId { service_fqin, service_unit_name: "test_service_unit".to_string() };

        add_implementation_to_sdv_type_mapping(
            &mut sdv_type_mapping,
            someip_service_instance.instance_id,
            &sdv_service_unit_id,
        );

        let sdv_type_parser = SdvTypeMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let mut protocol_mapping = ProtocolMapping::default();
        sdv_type_parser.process_sdv_type_mapping(
            &sdv_type_mapping,
            &mut protocol_mapping,
            &mut TypeMapping::default(),
        );

        let sdv_service_unit_mappings = protocol_mapping.sdv_service_units;

        assert_eq!(sdv_service_unit_mappings.len(), 1);
        let SdvIdentifier::ServiceUnitInfo { ref service_unit_id } =
            sdv_service_unit_mappings[0].sdv_identifier
        else {
            panic!("Unexpected identifier type")
        };
        assert_eq!(*service_unit_id, sdv_service_unit_id);
        assert_eq!(sdv_service_unit_mappings[0].unit_type, unit_type);

        assert_eq!(
            sdv_service_unit_mappings[0].someip_service_instance_info,
            someip_service_instance
        );
        assert_that!(
            sdv_service_unit_mappings[0].service_unit_interface,
            matches_pattern!(SdvServiceUnitInterface::RPC(unordered_elements_are!(
                eq(&RpcMethodMappingInfo {
                    sdv_method_name: rpc_method1.procedure.clone(),
                    someip_method_id: rpc_method1.method_id as u16
                }),
                eq(&RpcMethodMappingInfo {
                    sdv_method_name: rpc_method2.procedure.clone(),
                    someip_method_id: rpc_method2.method_id as u16
                }),
            )))
        );
    }

    #[test]
    fn process_sdv_type_mapping_detects_sdv_multipub_topic_unit_successfully() {
        let builder = UnitType::builder();
        let unit_type = builder
            .set_sdv_package_name("com.test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitType")
            .build()
            .expect("Failed to build Unit type!");
        let someip_service_instance = SomeIpService {
            service_id: 2,
            instance_id: 1,
            version: SomeIpServiceVersion { major: 1, minor: 2 },
        };

        let group_id: u16 = 1;
        let event_id: u16 = 2;

        let sdv_interface_type = SdvInterfaceType::OfferedTopic(Topic {
            topic_mapping: Some(Topic_mapping::SimpleMapping(SimpleTopicMapping {
                group_id: group_id as u32,
                event_id: event_id as u32,
                ..Default::default()
            })),
            multipub_topic_info: Some(MultipubTopicInfo {
                someip_instance_id: 1,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .into();

        let sdv_type_mapping =
            create_sdv_type_mapping(&unit_type, &someip_service_instance, sdv_interface_type);

        let sdv_type_parser = SdvTypeMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let mut protocol_mapping = ProtocolMapping::default();
        sdv_type_parser.process_sdv_type_mapping(
            &sdv_type_mapping,
            &mut protocol_mapping,
            &mut TypeMapping::default(),
        );

        let sdv_multipub_type_mapping = SdvMultipubMapping {
            unit_type,
            someip_service_instance_info: someip_service_instance,
            someip_event_id: event_id,
            someip_eventgroup_id: group_id,
        };

        let sdv_multipub_topic_mappings = protocol_mapping.sdv_multipub_topics;

        assert_eq!(sdv_multipub_topic_mappings.len(), 1);

        assert_eq!(sdv_multipub_topic_mappings[0], sdv_multipub_type_mapping);
    }

    fn create_vsidl_ff_method(
        service_id: u32,
        instance_id: u32,
        version_major: u32,
        version_minor: u32,
        method_id: u32,
    ) -> VsidlSomeIpFFMethod {
        VsidlSomeIpFFMethod {
            service_id,
            instance_id,
            version_major,
            version_minor,
            method_id,
            ..Default::default()
        }
    }

    #[test]
    fn process_sdv_type_mapping_detects_fire_and_forget_topic_successfully() {
        // GIVEN: UnitType and base SomeIpService for SdvTypeMapping
        let builder = UnitType::builder();
        let unit_type = builder
            .set_sdv_package_name("com.test.ff")
            .set_service_bundle_name("FFBundle")
            .set_unit_type_name("FFUnitType")
            .build()
            .expect("Failed to build Unit type!");

        let base_someip_service_for_sdv_type = SomeIpService {
            service_id: 100,
            instance_id: 0, // Placeholder, will be set by SdvServiceUnitMapping.instance_id
            version: SomeIpServiceVersion { major: 1, minor: 0 },
        };

        // GIVEN: SdvInterfaceType as OfferedTopic with FireAndForget enabled
        let sdv_interface_type = SdvInterfaceType::OfferedTopic(Topic {
            topic_mapping: Some(Topic_mapping::SomeipFfMethods(true)),
            ..Default::default()
        });

        // GIVEN: SdvTypeMapping
        let mut sdv_type_mapping = create_sdv_type_mapping(
            &unit_type,
            &base_someip_service_for_sdv_type,
            Some(sdv_interface_type),
        );

        // GIVEN: SdvServiceUnitId for the mapping
        let sdv_service_unit_id = SdvServiceUnitId {
            service_fqin: ServiceFqinInternal {
                sdv_vm_name: Some("test_vm_ff".to_string()),
                sdv_package_name: "com.test.ff".to_string(),
                service_bundle_name: "FFBundle".to_string(),
                service_instance_name: Some("instance_ff".to_string()),
            },
            service_unit_name: "ff_service_unit".to_string(),
        };

        // GIVEN: Fire and Forget methods for the SdvServiceUnitMapping
        let ff_method1_details = (10u32, 11u32, 1u32, 0u32, 100u32); // service_id, instance_id, major, minor, method_id
        let ff_method2_details = (20u32, 21u32, 2u32, 0u32, 200u32);

        let vsidl_ff_methods = vec![
            create_vsidl_ff_method(
                ff_method1_details.0,
                ff_method1_details.1,
                ff_method1_details.2,
                ff_method1_details.3,
                ff_method1_details.4,
            ),
            create_vsidl_ff_method(
                ff_method2_details.0,
                ff_method2_details.1,
                ff_method2_details.2,
                ff_method2_details.3,
                ff_method2_details.4,
            ),
        ];

        // GIVEN: SdvServiceUnitMapping with a specific instance_id for the SOME/IP service
        let sdv_unit_mapping_instance_id = 5u16;
        sdv_type_mapping.service_unit_mapping.push(VsidlSdvServiceUnitMapping {
            service_unit_id: Some(VsidlServiceUnitId {
                vm_name: sdv_service_unit_id
                    .service_fqin
                    .sdv_vm_name
                    .clone()
                    .unwrap_or_default()
                    .into(),
                package_name: sdv_service_unit_id.service_fqin.sdv_package_name.clone(),
                service_bundle_name: sdv_service_unit_id.service_fqin.service_bundle_name.clone(),
                service_instance_name: sdv_service_unit_id
                    .service_fqin
                    .service_instance_name
                    .clone()
                    .unwrap_or_default()
                    .into(),
                service_unit_name: sdv_service_unit_id.service_unit_name.clone(),
                ..Default::default()
            })
            .into(),
            instance_id: sdv_unit_mapping_instance_id as u32,
            fire_and_forget_method: vsidl_ff_methods.clone(),
            ..Default::default()
        });

        // WHEN: Processing the SdvTypeMapping
        let sdv_type_parser = SdvTypeMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let mut protocol_mapping = ProtocolMapping::default();
        let mut type_mapping = TypeMapping::default();
        sdv_type_parser.process_sdv_type_mapping(
            &sdv_type_mapping,
            &mut protocol_mapping,
            &mut type_mapping,
        );

        // THEN: Assert the results
        assert_eq!(
            protocol_mapping.sdv_service_units.len(),
            1,
            "Expected one SdvServiceUnitMapping"
        );
        let SdvIdentifier::ServiceUnitInfo { service_unit_id } =
            &protocol_mapping.sdv_service_units[0].sdv_identifier
        else {
            panic!("Unexpected identifier type")
        };

        assert_eq!(*service_unit_id, sdv_service_unit_id);
        assert_eq!(protocol_mapping.sdv_service_units[0].unit_type, unit_type);

        let expected_someip_instance_info = SomeIpService {
            service_id: base_someip_service_for_sdv_type.service_id,
            instance_id: sdv_unit_mapping_instance_id,
            version: base_someip_service_for_sdv_type.version,
        };
        assert_eq!(
            protocol_mapping.sdv_service_units[0].someip_service_instance_info,
            expected_someip_instance_info
        );

        match &protocol_mapping.sdv_service_units[0].service_unit_interface {
            SdvServiceUnitInterface::TOPIC(SdvTopicMappingInfo::FireAndForgetMapping {
                ff_mapping,
            }) => {
                assert_eq!(ff_mapping.len(), 2, "Expected two FireAndForget methods");

                let expected_ff_method1 = SomeIpMethod {
                    someip_service: SomeIpService {
                        service_id: ff_method1_details.0 as u16,
                        instance_id: ff_method1_details.1 as u16,
                        version: SomeIpServiceVersion {
                            major: ff_method1_details.2 as i8,
                            minor: ff_method1_details.3 as i32,
                        },
                    },
                    someip_method_id: ff_method1_details.4 as u16,
                };
                let expected_ff_method2 = SomeIpMethod {
                    someip_service: SomeIpService {
                        service_id: ff_method2_details.0 as u16,
                        instance_id: ff_method2_details.1 as u16,
                        version: SomeIpServiceVersion {
                            major: ff_method2_details.2 as i8,
                            minor: ff_method2_details.3 as i32,
                        },
                    },
                    someip_method_id: ff_method2_details.4 as u16,
                };

                assert_that!(
                    ff_mapping,
                    unordered_elements_are![eq(&expected_ff_method1), eq(&expected_ff_method2),]
                );
            }
            other => panic!("Expected FireAndForgetMapping, got {other:?}"),
        }
        assert!(type_mapping.topic_type_mapping.contains_key(&unit_type));
    }

    #[test]
    fn process_sdv_type_mapping_ff_false_no_service_unit_created() {
        let builder = UnitType::builder();
        let unit_type = builder
            .set_sdv_package_name("com.test.ff.false")
            .set_service_bundle_name("FFBundleFalse")
            .set_unit_type_name("FFUnitTypeFalse")
            .build()
            .unwrap();
        let someip_service_instance = SomeIpService {
            service_id: 101,
            instance_id: 0,
            version: SomeIpServiceVersion { major: 1, minor: 0 },
        };

        let sdv_interface_type = SdvInterfaceType::OfferedTopic(Topic {
            topic_mapping: Some(Topic_mapping::SomeipFfMethods(false)), // Key: FF is false
            ..Default::default()
        });

        let mut sdv_type_mapping =
            create_sdv_type_mapping(&unit_type, &someip_service_instance, Some(sdv_interface_type));
        // Add a service unit mapping, otherwise process_sdv_type_mapping bails early if service_unit_mapping is empty
        sdv_type_mapping.service_unit_mapping.push(VsidlSdvServiceUnitMapping {
            service_unit_id: Some(VsidlServiceUnitId {
                vm_name: Some("vm".to_string()),
                package_name: "com.test.ff.false".to_string(),
                service_bundle_name: "FFBundleFalse".to_string(),
                service_instance_name: Some("instance_false".to_string()),
                service_unit_name: "ff_false_unit".to_string(),
                ..Default::default()
            })
            .into(),
            instance_id: 1,
            ..Default::default()
        });

        let sdv_type_parser = SdvTypeMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let mut protocol_mapping = ProtocolMapping::default();
        let mut type_mapping = TypeMapping::default();

        sdv_type_parser.process_sdv_type_mapping(
            &sdv_type_mapping,
            &mut protocol_mapping,
            &mut type_mapping,
        );

        assert!(
            protocol_mapping.sdv_service_units.is_empty(),
            "Expected no service units when SomeipFfMethods is false"
        );
    }

    #[test]
    fn process_sdv_type_mapping_ff_skips_invalid_method_ids_in_ff_methods() {
        let builder = UnitType::builder();
        let unit_type = builder
            .set_sdv_package_name("com.test.ff.invalid")
            .set_service_bundle_name("FFBundleInvalid")
            .set_unit_type_name("FFUnitTypeInvalid")
            .build()
            .unwrap();
        let base_someip_service = SomeIpService {
            service_id: 102,
            instance_id: 0,
            version: SomeIpServiceVersion { major: 1, minor: 0 },
        };
        let sdv_interface_type = SdvInterfaceType::OfferedTopic(Topic {
            topic_mapping: Some(Topic_mapping::SomeipFfMethods(true)),
            ..Default::default()
        });
        let mut sdv_type_mapping =
            create_sdv_type_mapping(&unit_type, &base_someip_service, Some(sdv_interface_type));
        let sdv_service_unit_id = SdvServiceUnitId {
            service_fqin: ServiceFqinInternal {
                sdv_vm_name: Some("test_vm_ff_invalid".to_string()),
                sdv_package_name: "com.test.ff.invalid".to_string(),
                service_bundle_name: "FFBundleInvalid".to_string(),
                service_instance_name: Some("instance_ff_invalid".to_string()),
            },
            service_unit_name: "ff_service_unit_invalid".to_string(),
        };

        let valid_ff_method_details = (30u32, 31u32, 3u32, 0u32, 300u32);
        let invalid_method_id_ff_details = (40u32, 41u32, 4u32, 0u32, u16::MAX as u32 + 1); // Invalid method_id

        let vsidl_ff_methods = vec![
            create_vsidl_ff_method(
                valid_ff_method_details.0,
                valid_ff_method_details.1,
                valid_ff_method_details.2,
                valid_ff_method_details.3,
                valid_ff_method_details.4,
            ),
            create_vsidl_ff_method(
                invalid_method_id_ff_details.0,
                invalid_method_id_ff_details.1,
                invalid_method_id_ff_details.2,
                invalid_method_id_ff_details.3,
                invalid_method_id_ff_details.4,
            ),
        ];

        let sdv_unit_mapping_instance_id = 6u16;
        sdv_type_mapping.service_unit_mapping.push(VsidlSdvServiceUnitMapping {
            service_unit_id: Some(VsidlServiceUnitId {
                vm_name: sdv_service_unit_id
                    .service_fqin
                    .sdv_vm_name
                    .clone()
                    .unwrap_or_default()
                    .into(),
                package_name: sdv_service_unit_id.service_fqin.sdv_package_name.clone(),
                service_bundle_name: sdv_service_unit_id.service_fqin.service_bundle_name.clone(),
                service_instance_name: sdv_service_unit_id
                    .service_fqin
                    .service_instance_name
                    .clone()
                    .unwrap_or_default()
                    .into(),
                service_unit_name: sdv_service_unit_id.service_unit_name.clone(),
                ..Default::default()
            })
            .into(),
            instance_id: sdv_unit_mapping_instance_id as u32,
            fire_and_forget_method: vsidl_ff_methods,
            ..Default::default()
        });

        let sdv_type_parser = SdvTypeMappingParser::new(DEFAULT_SOMEIP_CONFIG);
        let mut protocol_mapping = ProtocolMapping::default();
        sdv_type_parser.process_sdv_type_mapping(
            &sdv_type_mapping,
            &mut protocol_mapping,
            &mut TypeMapping::default(),
        );

        assert_eq!(protocol_mapping.sdv_service_units.len(), 1);
        let mapped_unit = &protocol_mapping.sdv_service_units[0];
        match &mapped_unit.service_unit_interface {
            SdvServiceUnitInterface::TOPIC(SdvTopicMappingInfo::FireAndForgetMapping {
                ff_mapping,
            }) => {
                assert_eq!(
                    ff_mapping.len(),
                    1,
                    "Expected one valid FireAndForget method, invalid one should be skipped"
                );
                let expected_valid_ff_method = SomeIpMethod {
                    someip_service: SomeIpService {
                        service_id: valid_ff_method_details.0 as u16,
                        instance_id: valid_ff_method_details.1 as u16,
                        version: SomeIpServiceVersion {
                            major: valid_ff_method_details.2 as i8,
                            minor: valid_ff_method_details.3 as i32,
                        },
                    },
                    someip_method_id: valid_ff_method_details.4 as u16,
                };
                assert_eq!(ff_mapping[0], expected_valid_ff_method);
            }
            other => panic!("Expected FireAndForgetMapping, got {other:?}"),
        }
    }
}
