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

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;
use sdv_comms::sd::UnitType;
use sdv_config_provider::vsidl_provider_api::RpcMethodDescriptor;
use std::collections::HashMap;
use std::sync::Arc;

use sdv_comms::id::ServiceFqin;
use sdv_config_provider::vsidl_provider_api::{PublicationDescriptor, VsidlConfigProvider};

use crate::agent::protocol_mapping::MessageTranslationMode;
use crate::agent::protocol_mapping::ProtocolMapping;
use crate::agent::protocol_mapping::SdvServiceUnitInterface;
use crate::agent::protocol_mapping::SdvServiceUnitMapping;
use crate::agent::protocol_mapping::SomeIpEventMappingInfo;
use crate::agent::protocol_mapping::SomeIpEventMappingType;
use crate::agent::protocol_mapping::SomeIpRpcMappingInfo;
use crate::agent::protocol_mapping::SomeIpServiceInstanceMapping;
use crate::translator::direct::group_metadata::SomeIpGroupPublicationMetadata;
use crate::translator::direct::metadata::SomeIpMetadataBuilder;
use crate::translator::direct::metadata::SomeIpPublicationMetadata;
use crate::translator::direct::metadata::SomeIpRpcMetadata;
use crate::TypeMapping;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RpcMode {
    Request,
    Response,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SomeIpDescriptorId {
    SomeIpEventId { service_id: u16, major: u8, event_id: u16 },
    SomeIpMethodId { service_id: u16, major: u8, method_id: u16, mode: RpcMode },
}

impl SomeIpDescriptorId {
    pub(crate) fn new_event_id(service_id: u16, major: u8, event_id: u16) -> SomeIpDescriptorId {
        SomeIpDescriptorId::SomeIpEventId { service_id, major, event_id }
    }

    pub(crate) fn new_method_id(
        service_id: u16,
        major: u8,
        method_id: u16,
        mode: RpcMode,
    ) -> SomeIpDescriptorId {
        SomeIpDescriptorId::SomeIpMethodId { service_id, major, method_id, mode }
    }
}

pub(crate) struct SomeIpPublicationDescriptor {
    publication_message_size: u32,
    message_translation_mode: MessageTranslationMode,
}

impl SomeIpPublicationDescriptor {
    pub(crate) fn get_message_size(&self) -> u32 {
        self.publication_message_size
    }

    pub(crate) fn get_message_translation_mode(&self) -> MessageTranslationMode {
        self.message_translation_mode
    }
}

#[derive(Default)]
pub(crate) struct SomeIpDescriptorStore {
    someip_descriptors: HashMap<SomeIpDescriptorId, SomeIpPublicationDescriptor>,
    someip_metadata: HashMap<SomeIpDescriptorId, SomeIpPublicationMetadata>,
    someip_group_metadata: HashMap<SomeIpDescriptorId, SomeIpGroupPublicationMetadata>,
    someip_rpc_metadata: HashMap<SomeIpDescriptorId, SomeIpRpcMetadata>,
}

impl SomeIpDescriptorStore {
    #[tracing::instrument(skip_all)]
    pub(crate) async fn create(
        protocol_mapping: &ProtocolMapping,
        type_mapping: &TypeMapping,
        config_provider: &dyn VsidlConfigProvider,
        broker_fqin: &ServiceFqin,
    ) -> Self {
        let mut publication_by_unit_type = HashMap::new();
        let mut rpc_by_unit_type = HashMap::new();
        let mut descriptor_store = SomeIpDescriptorStore::default();
        Self::process_someip_service_instances(
            &protocol_mapping.someip_service_instances,
            type_mapping,
            &mut publication_by_unit_type,
            &mut rpc_by_unit_type,
            config_provider,
            broker_fqin,
            &mut descriptor_store,
        )
        .await;

        Self::process_sdv_service_units(
            &protocol_mapping.sdv_service_units,
            config_provider,
            type_mapping,
            broker_fqin,
            &mut descriptor_store,
        )
        .await;

        descriptor_store
    }

    async fn process_sdv_service_units(
        sdv_service_units: &[SdvServiceUnitMapping],
        config_provider: &dyn VsidlConfigProvider,
        type_mapping: &TypeMapping,
        broker_fqin: &ServiceFqin,
        descriptor_store: &mut SomeIpDescriptorStore,
    ) {
        for service_unit in sdv_service_units.iter() {
            if let SdvServiceUnitInterface::RPC(rpc_infos) = &service_unit.service_unit_interface {
                for rpc_info in rpc_infos.iter() {
                    let rpc_descriptor = match config_provider
                        .get_rpc_method_descriptor(
                            broker_fqin,
                            &service_unit.unit_type,
                            &rpc_info.sdv_method_name,
                        )
                        .await
                    {
                        Ok(descriptor) => descriptor,
                        Err(e) => {
                            log::error!(
                                    "Unable to build fetch rpc descriptor for unit type {:?} and method {:?}: {e}",
                                    service_unit.unit_type,
                                    rpc_info.sdv_method_name
                                );
                            continue;
                        }
                    };

                    let id = SomeIpDescriptorId::new_method_id(
                        service_unit.someip_service_instance_info.service_id,
                        service_unit.someip_service_instance_info.version.major as u8,
                        rpc_info.someip_method_id,
                        RpcMode::Request,
                    );
                    let Some(rpc_mapping) =
                        type_mapping.rpc_type_mapping.get(&service_unit.unit_type)
                    else {
                        log::error!("Unable to find SOME/IP type mapping for rpc interface {id:?}");
                        return;
                    };
                    let Some(method_mapping) = rpc_mapping.get(&rpc_info.sdv_method_name) else {
                        log::error!("Unable to find SOME/IP type mapping for method {id:?}");
                        continue;
                    };

                    match SomeIpMetadataBuilder::build_rpc_metadata(
                        rpc_descriptor.request_message.as_ref(),
                        method_mapping.0.type_mapping(),
                    ) {
                        Ok(metadata) => {
                            descriptor_store.someip_rpc_metadata.insert(id, metadata);
                        }
                        Err(e) => {
                            log::error!(
                                "Unable to build translation metadata for method {id:?} - {e:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    async fn process_someip_service_instances(
        someip_service_instances: &[SomeIpServiceInstanceMapping],
        type_mapping: &TypeMapping,
        publication_by_unit_type: &mut HashMap<UnitType, Arc<PublicationDescriptor>>,
        rpc_by_unit_type: &mut HashMap<(UnitType, String), Arc<RpcMethodDescriptor>>,
        config_provider: &dyn VsidlConfigProvider,
        broker_fqin: &ServiceFqin,
        descriptor_store: &mut SomeIpDescriptorStore,
    ) {
        for service_instance in someip_service_instances.iter() {
            for event in service_instance.event_infos.iter() {
                Self::process_someip_event(
                    event,
                    &service_instance.someip_service_instance_info.someip_service_instance,
                    type_mapping,
                    publication_by_unit_type,
                    config_provider,
                    broker_fqin,
                    descriptor_store,
                )
                .await;
            }
            for rpc_info in service_instance.rpc_infos.iter() {
                Self::process_someip_rpc(
                    rpc_info,
                    &service_instance.someip_service_instance_info.someip_service_instance,
                    type_mapping,
                    rpc_by_unit_type,
                    config_provider,
                    broker_fqin,
                    descriptor_store,
                )
                .await
            }
        }
    }

    async fn process_someip_rpc(
        rpc_info: &SomeIpRpcMappingInfo,
        someip_service: &SomeIpService,
        type_mapping: &TypeMapping,
        rpc_by_unit_type: &mut HashMap<(UnitType, String), Arc<RpcMethodDescriptor>>,
        config_provider: &dyn VsidlConfigProvider,
        broker_fqin: &ServiceFqin,
        descriptor_store: &mut SomeIpDescriptorStore,
    ) {
        for rpc_method in rpc_info.rpc_method_infos.iter() {
            let Some(rpc_descriptor) = Self::fetch_rpc_descriptor(
                rpc_by_unit_type,
                config_provider,
                broker_fqin,
                &rpc_info.broker_service_unit.unit_type,
                rpc_method.sdv_method_name.clone(),
            )
            .await
            else {
                log::error!(
                    "Unable to build fetch rpc descriptor for unit type {}",
                    rpc_info.broker_service_unit.unit_type
                );
                continue;
            };

            let id = SomeIpDescriptorId::new_method_id(
                someip_service.service_id,
                someip_service.version.major as u8,
                rpc_method.someip_method_id,
                RpcMode::Response,
            );
            let Some(rpc_mapping) =
                type_mapping.rpc_type_mapping.get(&rpc_info.broker_service_unit.unit_type)
            else {
                log::error!("Unable to find SOME/IP type mapping for rpc interface {id:?}");
                return;
            };
            let Some(method_mapping) = rpc_mapping.get(&rpc_method.sdv_method_name) else {
                log::error!("Unable to find SOME/IP type mapping for method {id:?}");
                continue;
            };

            match SomeIpMetadataBuilder::build_rpc_metadata(
                rpc_descriptor.response_message.as_ref(),
                method_mapping.1.type_mapping(),
            ) {
                Ok(metadata) => {
                    descriptor_store.someip_rpc_metadata.insert(id, metadata);
                }
                Err(e) => {
                    log::error!("Unable to build translation metadata for method {id:?} - {e:?}");
                }
            }
        }
    }

    async fn process_someip_event(
        event: &SomeIpEventMappingInfo,
        someip_service: &SomeIpService,
        type_mapping: &TypeMapping,
        publication_by_unit_type: &mut HashMap<UnitType, Arc<PublicationDescriptor>>,
        config_provider: &dyn VsidlConfigProvider,
        broker_fqin: &ServiceFqin,
        descriptor_store: &mut SomeIpDescriptorStore,
    ) {
        match &event.event_mapping_type {
            SomeIpEventMappingType::SIMPLE(broker_service_unit) => {
                let Some(publication_descriptor) = Self::fetch_descriptor(
                    publication_by_unit_type,
                    config_provider,
                    broker_fqin,
                    &broker_service_unit.unit_type,
                )
                .await
                else {
                    log::error!(
                        "Unable to build fetch publication descriptor for unit type {}",
                        broker_service_unit.unit_type
                    );
                    return;
                };
                let id = SomeIpDescriptorId::new_event_id(
                    someip_service.service_id,
                    someip_service.version.major as u8,
                    event.event_id,
                );
                let Some(type_mapping) =
                    type_mapping.topic_type_mapping.get(&broker_service_unit.unit_type)
                else {
                    log::error!("Unable to find SOME/IP type mapping for event {id:?}");
                    return;
                };
                if type_mapping.message_translation_mode() != MessageTranslationMode::Reflection {
                    descriptor_store.someip_descriptors.insert(
                        id,
                        SomeIpPublicationDescriptor {
                            publication_message_size: publication_descriptor
                                .size_metadata
                                .message_size,
                            message_translation_mode: type_mapping.message_translation_mode(),
                        },
                    );
                } else {
                    let Ok(publication_metadata) = SomeIpPublicationMetadata::builder()
                        .build_publication_metadata(&publication_descriptor, type_mapping)
                    else {
                        log::error!("Unable to build translation metadata for event {id:?}");
                        return;
                    };
                    descriptor_store.someip_metadata.insert(id, publication_metadata);
                }
            }
            SomeIpEventMappingType::GROUP(event_definition, service_unit_infos) => {
                let mut group_part_publication_descriptors_with_arc = Vec::new();
                for (broker_service_unit, field_name_mapping) in service_unit_infos.iter() {
                    let Some(publication_descriptor) = Self::fetch_descriptor(
                        publication_by_unit_type,
                        config_provider,
                        broker_fqin,
                        &broker_service_unit.unit_type,
                    )
                    .await
                    else {
                        log::error!(
                            "Unable to build fetch publication descriptor for unit type {}",
                            broker_service_unit.unit_type
                        );
                        continue;
                    };

                    group_part_publication_descriptors_with_arc
                        .push((publication_descriptor, field_name_mapping));
                }
                let group_part_publication_descriptors: Vec<(
                    &PublicationDescriptor,
                    &HashMap<String, String>,
                )> = group_part_publication_descriptors_with_arc
                    .iter()
                    .map(|(pd, fm)| (&**pd, *fm))
                    .collect();

                let event_id = SomeIpDescriptorId::new_event_id(
                    someip_service.service_id,
                    someip_service.version.major as u8,
                    event.event_id,
                );
                let Ok(group_publication_metadata) = SomeIpGroupPublicationMetadata::builder()
                    .build_group_publication_metadata(
                        event_definition,
                        &group_part_publication_descriptors,
                    )
                else {
                    log::error!(
                        "Unable to build group publication metadata for event {event_id:?}"
                    );
                    return;
                };
                descriptor_store.someip_group_metadata.insert(event_id, group_publication_metadata);
            }
        }
    }

    pub(crate) fn get_descriptor(
        &self,
        descriptor_id: &SomeIpDescriptorId,
    ) -> Option<&SomeIpPublicationDescriptor> {
        self.someip_descriptors.get(descriptor_id)
    }

    pub(crate) fn get_metadata(
        &self,
        descriptor_id: &SomeIpDescriptorId,
    ) -> Option<&SomeIpPublicationMetadata> {
        self.someip_metadata.get(descriptor_id)
    }

    #[allow(dead_code)]
    pub(crate) fn get_rpc_metadata(
        &self,
        descriptor_id: &SomeIpDescriptorId,
    ) -> Option<&SomeIpRpcMetadata> {
        self.someip_rpc_metadata.get(descriptor_id)
    }

    pub(crate) fn get_group_metadata(
        &self,
        descriptor_id: &SomeIpDescriptorId,
    ) -> Option<&SomeIpGroupPublicationMetadata> {
        self.someip_group_metadata.get(descriptor_id)
    }

    async fn fetch_rpc_descriptor(
        rpc_by_unit_type: &mut HashMap<(UnitType, String), Arc<RpcMethodDescriptor>>,
        config_provider: &dyn VsidlConfigProvider,
        broker_fqin: &ServiceFqin,
        unit_type: &UnitType,
        method_name: String,
    ) -> Option<Arc<RpcMethodDescriptor>> {
        match rpc_by_unit_type.get(&(unit_type.clone(), method_name.clone())) {
            Some(rpc_descriptor) => Some(rpc_descriptor.clone()),
            None => {
                if let Ok(descriptor) = config_provider
                    .get_rpc_method_descriptor(broker_fqin, unit_type, &method_name)
                    .await
                {
                    let value = Arc::new(descriptor);
                    rpc_by_unit_type
                        .insert((unit_type.clone(), method_name.clone()), value.clone());
                    Some(value)
                } else {
                    None
                }
            }
        }
    }

    async fn fetch_descriptor(
        publication_by_unit_type: &mut HashMap<UnitType, Arc<PublicationDescriptor>>,
        config_provider: &dyn VsidlConfigProvider,
        broker_fqin: &ServiceFqin,
        unit_type: &UnitType,
    ) -> Option<Arc<PublicationDescriptor>> {
        match publication_by_unit_type.get(unit_type) {
            Some(publication_descriptor) => Some(publication_descriptor.clone()),
            None => {
                if let Ok(descriptor) =
                    config_provider.get_publication_descriptor(broker_fqin, unit_type).await
                {
                    let value = Arc::new(descriptor);
                    publication_by_unit_type.insert(unit_type.clone(), value.clone());
                    Some(value)
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::agent::protocol_mapping::BrokerServiceUnit;
    use crate::agent::protocol_mapping::RpcMethodMappingInfo;
    use crate::agent::protocol_mapping::SomeIpEventMappingInfo;
    use crate::agent::protocol_mapping::SomeIpServiceInstanceInfo;
    use crate::agent::protocol_mapping::SomeIpServiceInstanceMapping;
    use crate::agent::protocol_mapping::SomeIpTypeMapping;

    use google_sdv_someip::aidl::google::sdv::someip::{
        SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
    };
    use protobuf::MessageFull;
    use sdv_comms::id::ServiceFqin;
    use sdv_comms::sd::UnitType;
    use sdv_config_provider::vsidl_provider_api::{
        MockVsidlConfigProvider, PublicationDescriptor, SizeMetadata,
    };
    use sdv_core_core_services_unit_someip_broker_test_test_protos::message::SimpleTestMessage;
    use sdv_status_rs::{SdvStatus, SdvStatusCode};
    use someip_config::broker_params::SdMinorVersionMode;
    use someip_config::serde_config::ffi::LenFieldSize;
    use someip_serde_reflect::{
        SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
        SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
    };

    fn create_simple_event_mapping(
        service_unit_name: &str,
        unit_type: UnitType,
    ) -> SomeIpEventMappingType {
        SomeIpEventMappingType::SIMPLE(Box::new(BrokerServiceUnit {
            service_unit_name: service_unit_name.to_string(),
            pub_metadata: None,
            unit_type,
        }))
    }

    fn create_group_event_mapping(
        group_event_definition: Option<SomeIpStructTypeDefinition>,
        service_units: Vec<(String, UnitType, HashMap<String, String>)>,
    ) -> SomeIpEventMappingType {
        let service_unit_infos = service_units
            .into_iter()
            .map(|(name, unit_type, field_name_mapping)| {
                (
                    BrokerServiceUnit { service_unit_name: name, pub_metadata: None, unit_type },
                    field_name_mapping,
                )
            })
            .collect();
        SomeIpEventMappingType::GROUP(group_event_definition, service_unit_infos)
    }

    fn create_test_service_instance(
        service_id: u16,
        major: u8,
        minor: u32,
        event_id: u16,
        eventgroup_id: u16,
        event_mapping_type: SomeIpEventMappingType,
    ) -> SomeIpServiceInstanceMapping {
        SomeIpServiceInstanceMapping {
            someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                SomeIpService {
                    service_id,
                    instance_id: 0,
                    version: SomeIpServiceVersion { major: major as i8, minor: minor as i32 },
                },
                SdMinorVersionMode::ExactOrAnyMinorVersion,
            ),
            event_infos: vec![SomeIpEventMappingInfo {
                event_id,
                eventgroup_id,
                event_mapping_type,
            }],
            rpc_infos: vec![],
        }
    }

    fn create_simple_message_someip_mapping(
        message_translation_mode: MessageTranslationMode,
    ) -> SomeIpTypeMapping {
        SomeIpTypeMapping::with_mode(
            Some(SomeIpStructTypeDefinition {
                name: String::new(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: vec![
                    SomeIpStructFieldTypeDefinition {
                        data_tag: None,
                        field_name: "a".to_string(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U64,
                        ),
                    },
                    SomeIpStructFieldTypeDefinition {
                        data_tag: None,
                        field_name: "b".to_string(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::F64,
                        ),
                    },
                    SomeIpStructFieldTypeDefinition {
                        data_tag: None,
                        field_name: "c".to_string(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U32,
                        ),
                    },
                ],
            }),
            message_translation_mode,
        )
    }

    #[tokio::test]
    async fn test_create_descriptor_store_skips_missing_publication_descriptor() {
        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("Unit Type should be created");

        let simple_event_mapping = create_simple_event_mapping("test-service-unit", unit_type);
        let service_instance =
            create_test_service_instance(100, 1, 0, 200, 300, simple_event_mapping);

        let mut vsidl_provider = MockVsidlConfigProvider::new();
        vsidl_provider
            .expect_get_publication_descriptor()
            .times(1)
            .returning(|_, _| Err(SdvStatus::new(SdvStatusCode::NotFound)));

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Unit Type should be created");

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![service_instance],
            ..Default::default()
        };
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &TypeMapping::default(),
            &vsidl_provider,
            &broker_fqin,
        )
        .await;

        assert_eq!(descriptor_store.someip_descriptors.len(), 0);
        assert_eq!(descriptor_store.someip_metadata.len(), 0);
    }

    #[tokio::test]
    async fn test_create_and_get_simple_event_descriptor() {
        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("Unit Type should be created");

        let simple_event_mapping =
            create_simple_event_mapping("test-service-unit", unit_type.clone());
        let service_instance =
            create_test_service_instance(100, 1, 0, 200, 300, simple_event_mapping);

        let mut type_mapping = TypeMapping::default();
        type_mapping.topic_type_mapping.insert(
            unit_type.clone(),
            create_simple_message_someip_mapping(MessageTranslationMode::DynamicLibrary),
        );

        let message_size = 25;
        let mut vsidl_provider = MockVsidlConfigProvider::new();
        let publication_descriptor = PublicationDescriptor {
            message_descriptor: SimpleTestMessage::descriptor(),
            size_metadata: SizeMetadata { message_size, ..Default::default() },
        };
        vsidl_provider.expect_get_publication_descriptor().times(1).returning(move |_, ut| {
            if *ut == unit_type {
                Ok(publication_descriptor.clone())
            } else {
                Err(SdvStatus::new(SdvStatusCode::NotFound))
            }
        });

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Unit Type should be created");

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![service_instance],
            ..Default::default()
        };
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &vsidl_provider,
            &broker_fqin,
        )
        .await;

        assert_eq!(descriptor_store.someip_descriptors.len(), 1);
        assert_eq!(descriptor_store.someip_metadata.len(), 0);

        let event_id = SomeIpDescriptorId::new_event_id(100, 1, 200);
        let retrieved_descriptor =
            descriptor_store.get_descriptor(&event_id).expect("Descriptor should be created");
        assert_eq!(retrieved_descriptor.get_message_size(), message_size);

        let non_existent_id = SomeIpDescriptorId::new_event_id(999, 9, 999);
        assert!(descriptor_store.get_descriptor(&non_existent_id).is_none());
    }

    #[tokio::test]
    async fn test_create_and_get_simple_event_metadata() {
        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("Unit Type should be created");

        let simple_event_mapping =
            create_simple_event_mapping("test-service-unit", unit_type.clone());
        let service_instance =
            create_test_service_instance(100, 1, 0, 200, 300, simple_event_mapping);

        let mut type_mapping = TypeMapping::default();
        type_mapping.topic_type_mapping.insert(
            unit_type.clone(),
            create_simple_message_someip_mapping(MessageTranslationMode::Reflection),
        );

        let message_size = 25;
        let mut vsidl_provider = MockVsidlConfigProvider::new();
        let publication_descriptor = PublicationDescriptor {
            message_descriptor: SimpleTestMessage::descriptor(),
            size_metadata: SizeMetadata { message_size, ..Default::default() },
        };
        vsidl_provider.expect_get_publication_descriptor().times(1).returning(move |_, ut| {
            if *ut == unit_type {
                Ok(publication_descriptor.clone())
            } else {
                Err(SdvStatus::new(SdvStatusCode::NotFound))
            }
        });

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Unit Type should be created");

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![service_instance],
            ..Default::default()
        };
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &vsidl_provider,
            &broker_fqin,
        )
        .await;

        assert_eq!(descriptor_store.someip_descriptors.len(), 0);
        assert_eq!(descriptor_store.someip_metadata.len(), 1);

        let event_id = SomeIpDescriptorId::new_event_id(100, 1, 200);
        let retrieved_metadata =
            descriptor_store.get_metadata(&event_id).expect("Metadata should be created");
        assert_eq!(retrieved_metadata.sdv_publication_size(), message_size as usize);
        assert_eq!(retrieved_metadata.translation_metadata().fields.len(), 3);

        let non_existent_id = SomeIpDescriptorId::new_event_id(999, 9, 999);
        assert!(descriptor_store.get_metadata(&non_existent_id).is_none());
    }

    #[tokio::test]
    async fn test_create_and_get_group_event_metadata() {
        let unit_type1 = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitType1")
            .build()
            .unwrap();
        let unit_type2 = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitType2")
            .build()
            .unwrap();

        let group_event_definition = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "part1".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U64,
                    ),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "part2".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::F64,
                    ),
                },
            ],
        };

        let group_event_mapping = create_group_event_mapping(
            Some(group_event_definition),
            vec![
                (
                    "part1".to_string(),
                    unit_type1.clone(),
                    HashMap::from([("part1".to_string(), "a".to_string())]),
                ),
                (
                    "part2".to_string(),
                    unit_type2.clone(),
                    HashMap::from([("part2".to_string(), "b".to_string())]),
                ),
            ],
        );
        let service_instance =
            create_test_service_instance(100, 1, 0, 200, 300, group_event_mapping);

        let mut vsidl_provider = MockVsidlConfigProvider::new();
        let publication_descriptor1 = PublicationDescriptor {
            message_descriptor: SimpleTestMessage::descriptor(),
            size_metadata: SizeMetadata { message_size: 10, ..Default::default() },
        };
        let publication_descriptor2 = PublicationDescriptor {
            message_descriptor: SimpleTestMessage::descriptor(),
            size_metadata: SizeMetadata { message_size: 20, ..Default::default() },
        };

        let pd1_clone = publication_descriptor1.clone();
        let pd2_clone = publication_descriptor2.clone();
        vsidl_provider.expect_get_publication_descriptor().times(2).returning(move |_, ut| {
            if *ut == unit_type1 {
                Ok(pd1_clone.clone())
            } else if *ut == unit_type2 {
                Ok(pd2_clone.clone())
            } else {
                Err(SdvStatus::new(SdvStatusCode::NotFound))
            }
        });

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .unwrap();

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![service_instance],
            ..Default::default()
        };
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &TypeMapping::default(),
            &vsidl_provider,
            &broker_fqin,
        )
        .await;

        assert_eq!(descriptor_store.someip_group_metadata.len(), 1);

        let id = SomeIpDescriptorId::new_event_id(100, 1, 200);

        let metadata = descriptor_store.get_group_metadata(&id);
        assert!(metadata.is_some());
        assert_eq!(metadata.unwrap().get_group_parts_metadata().len(), 2);
        assert_eq!(
            metadata.unwrap().get_group_parts_metadata()[0].get_metadata().sdv_publication_size(),
            10
        );
        assert_eq!(
            metadata.unwrap().get_group_parts_metadata()[1].get_metadata().sdv_publication_size(),
            20
        );
    }

    #[tokio::test]
    async fn test_create_descriptor_store_skips_missing_group_publication_descriptor() {
        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .unwrap();

        let group_event_mapping = create_group_event_mapping(
            None,
            vec![("part1".to_string(), unit_type, HashMap::new())],
        );
        let service_instance =
            create_test_service_instance(100, 1, 0, 200, 300, group_event_mapping);

        let mut vsidl_provider = MockVsidlConfigProvider::new();
        vsidl_provider
            .expect_get_publication_descriptor()
            .times(1)
            .returning(|_, _| Err(SdvStatus::new(SdvStatusCode::NotFound)));

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .unwrap();

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![service_instance],
            ..Default::default()
        };
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &TypeMapping::default(),
            &vsidl_provider,
            &broker_fqin,
        )
        .await;

        assert_eq!(descriptor_store.someip_descriptors.len(), 0);
    }

    #[tokio::test]
    async fn test_create_someip_rpc_metadata() {
        // ARRANGE
        let service_id = 100;
        let major_version = 1;
        let method_id = 42;
        let method_name = "TestMethod".to_string();

        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnit")
            .build()
            .unwrap();

        let rpc_info = SomeIpRpcMappingInfo {
            broker_service_unit: BrokerServiceUnit {
                service_unit_name: "test_unit".to_string(),
                pub_metadata: None,
                unit_type: unit_type.clone(),
            },
            rpc_method_infos: vec![RpcMethodMappingInfo {
                sdv_method_name: method_name.clone(),
                someip_method_id: method_id,
            }],
        };

        let service_instance = SomeIpServiceInstanceMapping {
            someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                SomeIpService {
                    service_id,
                    instance_id: 1,
                    version: SomeIpServiceVersion { major: major_version as i8, minor: 0 },
                },
                SdMinorVersionMode::ExactOrAnyMinorVersion,
            ),
            rpc_infos: vec![rpc_info],
            ..Default::default()
        };

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![service_instance],
            ..Default::default()
        };

        let mut type_mapping = TypeMapping::default();
        let someip_type_mapping =
            SomeIpTypeMapping::new(Some(get_simple_test_message_someip_def()));
        let mut rpc_method_map = HashMap::new();
        rpc_method_map.insert(
            method_name.clone(),
            (SomeIpTypeMapping::new(None), someip_type_mapping.clone()),
        );
        type_mapping.rpc_type_mapping.insert(unit_type.clone(), rpc_method_map);

        let mut config_provider = MockVsidlConfigProvider::new();
        config_provider.expect_get_rpc_method_descriptor().returning(move |_, _, _| {
            Ok(RpcMethodDescriptor {
                request_message: None,
                response_message: Some(SimpleTestMessage::descriptor()),
            })
        });

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.sdv.test")
            .service_bundle_name("TestBundle")
            .service_instance_name("instance")
            .build()
            .expect("Service FQIN must be created");

        // ACT
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &config_provider,
            &broker_fqin,
        )
        .await;

        // ASSERT
        assert_eq!(descriptor_store.someip_rpc_metadata.len(), 1);
        let id = SomeIpDescriptorId::new_method_id(
            service_id,
            major_version,
            method_id,
            RpcMode::Response,
        );
        assert!(descriptor_store.someip_rpc_metadata.contains_key(&id));
        let metadata = descriptor_store.get_rpc_metadata(&id).unwrap();
        assert_eq!(metadata.get_translation_metadata().fields.len(), 3);
    }

    #[tokio::test]
    async fn test_create_sdv_rpc_metadata() {
        // ARRANGE
        let service_id = 100;
        let major_version = 1;
        let method_id = 42;
        let method_name = "TestMethod".to_string();

        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnit")
            .build()
            .unwrap();

        let sdv_service_unit = SdvServiceUnitMapping {
            sdv_identifier: crate::agent::protocol_mapping::SdvIdentifier::ServiceUnitInfo {
                service_unit_id: crate::agent::protocol_mapping::SdvServiceUnitId {
                    service_fqin: crate::agent::protocol_mapping::ServiceFqinInternal {
                        sdv_vm_name: None,
                        sdv_package_name: "a".to_string(),
                        service_bundle_name: "b".to_string(),
                        service_instance_name: None,
                    },
                    service_unit_name: "c".to_string(),
                },
            },
            unit_type: unit_type.clone(),
            someip_service_instance_info: SomeIpService {
                service_id,
                instance_id: 1,
                version: SomeIpServiceVersion { major: major_version as i8, minor: 0 },
            },
            service_unit_interface: SdvServiceUnitInterface::RPC(vec![RpcMethodMappingInfo {
                sdv_method_name: method_name.clone(),
                someip_method_id: method_id,
            }]),
        };

        let protocol_mapping =
            ProtocolMapping { sdv_service_units: vec![sdv_service_unit], ..Default::default() };

        let mut type_mapping = TypeMapping::default();
        let someip_type_mapping =
            SomeIpTypeMapping::new(Some(get_simple_test_message_someip_def()));
        let mut rpc_method_map = HashMap::new();
        rpc_method_map.insert(
            method_name.clone(),
            (someip_type_mapping.clone(), SomeIpTypeMapping::new(None)),
        );
        type_mapping.rpc_type_mapping.insert(unit_type.clone(), rpc_method_map);

        let mut config_provider = MockVsidlConfigProvider::new();
        config_provider.expect_get_rpc_method_descriptor().returning(move |_, _, _| {
            Ok(RpcMethodDescriptor {
                request_message: Some(SimpleTestMessage::descriptor()),
                response_message: None,
            })
        });

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.sdv.test")
            .service_bundle_name("TestBundle")
            .service_instance_name("instance")
            .build()
            .expect("Service FQIN must be created");

        // ACT
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &config_provider,
            &broker_fqin,
        )
        .await;

        // ASSERT
        assert_eq!(descriptor_store.someip_rpc_metadata.len(), 1);
        let id = SomeIpDescriptorId::new_method_id(
            service_id,
            major_version,
            method_id,
            RpcMode::Request,
        );
        assert!(descriptor_store.someip_rpc_metadata.contains_key(&id));
        let metadata = descriptor_store.get_rpc_metadata(&id).unwrap();
        assert_eq!(metadata.get_translation_metadata().fields.len(), 3);
    }

    fn get_simple_test_message_someip_def() -> SomeIpStructTypeDefinition {
        SomeIpStructTypeDefinition {
            name: "SimpleTestMessageSomeIp".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "a".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U64,
                    ),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "b".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::F64,
                    ),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "c".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U8,
                    ),
                },
            ],
        }
    }
}
