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

use byteorder::ByteOrder;
use mockall::automock;

use protobuf::reflect::MessageDescriptor;
use sdv_comms::id::ServiceFqin;
use sdv_comms::sd::UnitType;
use sdv_config_provider::vsidl_provider_api::VsidlConfigProviderV1;
use sdv_vsidl_common::proto_size_utils::compute_length_size;
use sdv_vsidl_common::proto_size_utils::compute_message_length_including_header;
use someip_config::serde_config::ffi::SomeIpConfig;
use someip_serde_reflect::SomeIpStructTypeDefinition;
use std::sync::Arc;
use tokio::time::Instant;

use crate::agent::protocol_mapping::MessageTranslationMode;
use crate::agent::protocol_mapping::SomeIpTypeMapping;
use crate::agent::protocol_mapping::TypeMapping;
use crate::benchmark::benchmark_api::IBenchmark;
use crate::translator::direct::DirectTranslator;
use crate::translator::direct::ProtoFormatData;
use crate::translator::TranslationError;
use crate::translator::{SomeIpDescriptorId, SomeIpDescriptorStore};

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum TranslationMode {
    // Translate a message corresponding to a DataTunnel topic
    Topic,
    // Translate a message corresponding to a RPC method request parameter
    // The method name is specified in the variant
    RpcRequest(String),
    // Translate a message corresponding to a RPC method response parameter
    // The method name is specified in the variant
    RpcResponse(String),
}

#[automock]
#[async_trait::async_trait]
pub(crate) trait IBrokerTranslator: Send + Sync {
    fn translate_someip_group_event(
        &self,
        buffer: &[u8],
        someip_descriptor_id: &SomeIpDescriptorId,
    ) -> anyhow::Result<Vec<anyhow::Result<ProtoFormatData>>>;

    fn translate_from_someip_to_protobuf_topic(
        &self,
        unit_type: &UnitType,
        someip_message: &[u8],
        someip_descriptor_id: &SomeIpDescriptorId,
    ) -> anyhow::Result<ProtoFormatData>;

    async fn translate_from_someip_to_protobuf_rpc(
        &self,
        unit_type: &UnitType,
        someip_message: &[u8],
        someip_descriptor_id: &SomeIpDescriptorId,
    ) -> anyhow::Result<ProtoFormatData>;

    async fn translate_from_protobuf_to_someip(
        &self,
        service_fqin: &ServiceFqin,
        unit_type: &UnitType,
        message_data: &[u8],
        translation_mode: TranslationMode,
    ) -> anyhow::Result<Vec<u8>>;

    async fn get_publication_size_metadata(
        &self,
        service_fqin: &ServiceFqin,
        unit_type: &UnitType,
    ) -> anyhow::Result<(u32, u32)>;
}

pub(crate) struct BrokerTranslator<B: ByteOrder + Send + Sync> {
    config_provider: Box<dyn VsidlConfigProviderV1 + Send + Sync>,
    descriptor_store: SomeIpDescriptorStore,
    type_mapping: TypeMapping,
    someip_config: SomeIpConfig,
    someip_to_protobuf_direct_translator: DirectTranslator<B>,
    benchmark: Option<Arc<dyn IBenchmark<UnitType>>>,
}

impl<B: ByteOrder + Send + Sync> BrokerTranslator<B> {
    pub(crate) fn new(
        config_provider: Box<dyn VsidlConfigProviderV1 + Send + Sync>,
        descriptor_store: SomeIpDescriptorStore,
        type_mapping: TypeMapping,
        someip_config: SomeIpConfig,
        someip_to_protobuf_direct_translator: DirectTranslator<B>,
        benchmark: Option<Arc<dyn IBenchmark<UnitType>>>,
    ) -> Self {
        BrokerTranslator {
            config_provider,
            descriptor_store,
            type_mapping,
            someip_to_protobuf_direct_translator,
            someip_config,
            benchmark,
        }
    }

    fn translate_rpc_protobuf_to_someip(
        &self,
        protobuf_message_data: &[u8],
        rpc_message_descriptor: &Option<MessageDescriptor>,
        someip_message_definition: Option<&SomeIpStructTypeDefinition>,
    ) -> anyhow::Result<Vec<u8>> {
        let mut is = protobuf::CodedInputStream::from_bytes(protobuf_message_data);

        match (someip_message_definition, rpc_message_descriptor) {
            (Some(message_definition), Some(message_descriptor)) => {
                let protobuf_message = message_descriptor.parse_from(&mut is)?;
                Ok(someip_serde::serializer::get_someip_bytes_from_message_dyn(
                    self.someip_config,
                    message_definition,
                    message_descriptor,
                    &*protobuf_message,
                )?)
            }
            (None, None) => Ok(Vec::new()),
            (None, Some(_)) => Err(anyhow::Error::msg("Missing SOME/IP type definition")),
            (Some(_), None) => Err(anyhow::Error::msg("Missing SDV type definition")),
        }
    }

    fn get_someip_definition<'lt>(
        &'lt self,
        unit_type: &UnitType,
        translation_mode: TranslationMode,
    ) -> anyhow::Result<&'lt SomeIpTypeMapping> {
        match translation_mode {
            TranslationMode::Topic => {
                let Some(struct_type_definition) =
                    self.type_mapping.topic_type_mapping.get(unit_type)
                else {
                    return Err(anyhow::anyhow!(
                        "Unable to find SOME/IP type definition for unit type {unit_type}",
                    ));
                };
                Ok(struct_type_definition)
            }
            TranslationMode::RpcRequest(method) => {
                let Some(method_map) = self.type_mapping.rpc_type_mapping.get(unit_type) else {
                    return Err(anyhow::anyhow!(
                        "Unable to find SOME/IP type definition for unit type {unit_type}",
                    ));
                };
                let Some(rpc_metadata) = method_map.get(&method) else {
                    return Err(anyhow::anyhow!("Unable to find SOME/IP type definition for method {method} in unit type {unit_type}"));
                };
                Ok(&rpc_metadata.0)
            }
            TranslationMode::RpcResponse(method) => {
                let Some(method_map) = self.type_mapping.rpc_type_mapping.get(unit_type) else {
                    return Err(anyhow::anyhow!(
                        "Unable to find SOME/IP type definition for unit type {unit_type}",
                    ));
                };
                let Some(rpc_metadata) = method_map.get(&method) else {
                    return Err(anyhow::anyhow!("Unable to find SOME/IP type definition for method {method} in unit type {unit_type}"));
                };
                Ok(&rpc_metadata.1)
            }
        }
    }

    fn encode_varint32(mut value: u32, buffer: &mut [u8]) {
        // A u32 varint can take at most 5 bytes.
        debug_assert!(buffer.len() >= 5, "Buffer too small for varint encoding");
        let mut i = 0;
        while value > 0x7F {
            // Encode the least significant 7 bits (& 0x7F), set MSB to 1 (| 0x80) to indicate more bytes will follow
            buffer[i] = (value as u8 & 0x7F) | 0x80;
            value >>= 7;
            i += 1;
        }
        // Final byte (value) is < 0x80 (guaranteed by the while loop), so no info is lost when casting to u8.
        // MSB is already correctly set to 0
        buffer[i] = value as u8;
    }
}

#[async_trait::async_trait]
impl<B: ByteOrder + Send + Sync> IBrokerTranslator for BrokerTranslator<B> {
    #[tracing::instrument(skip_all)]
    fn translate_someip_group_event(
        &self,
        buffer: &[u8],
        someip_descriptor_id: &SomeIpDescriptorId,
    ) -> anyhow::Result<Vec<anyhow::Result<ProtoFormatData>>> {
        let Some(translation_metadata) =
            self.descriptor_store.get_group_metadata(someip_descriptor_id)
        else {
            return Err(anyhow::anyhow!(
                "No translation table exists for event {:?}",
                someip_descriptor_id
            ));
        };

        self.someip_to_protobuf_direct_translator
            .translate_group_publications(buffer, translation_metadata)
    }

    #[tracing::instrument(skip_all, fields(someip_descriptor_id))]
    fn translate_from_someip_to_protobuf_topic(
        &self,
        unit_type: &UnitType,
        someip_message: &[u8],
        someip_descriptor_id: &SomeIpDescriptorId,
    ) -> anyhow::Result<ProtoFormatData> {
        //TODO(b/442393195): Add benchmark measurements to the direct translation method
        let time_at_fn_start = Instant::now();
        if let Some(translation_metadata) = self.descriptor_store.get_metadata(someip_descriptor_id)
        {
            self.someip_to_protobuf_direct_translator
                .translate_publication(someip_message, translation_metadata)
        } else {
            let Some(publication_descriptor) =
                self.descriptor_store.get_descriptor(someip_descriptor_id)
            else {
                return Err(anyhow::anyhow!(
                    "No publication metadata exists for event {:?}",
                    someip_descriptor_id
                ));
            };
            let size =
                compute_message_length_including_header(publication_descriptor.get_message_size())
                    as usize;
            if publication_descriptor.get_message_translation_mode()
                == MessageTranslationMode::DynamicLibrary
            {
                return Err(anyhow::anyhow!("Dynamic library loading is deprecated for SOME/IP translation, use REFLECTION or INTERPRET_AS_BYTES instead"));
            }
            // Manually construct a length-delimited protobuf message containing a single bytes field.
            // The final buffer structure is:
            // [total_message_length_varint]
            // [field_tag_varint]
            // [field_data_length_varint]
            // [raw_someip_data]
            // This is done manually instead of relying on third-party structs (CodedOutputStream for example)
            // to avoid any performance overhead and ties to any specific library

            // Protobuf field with number = 1, wire type = LengthDelimited (2)
            const PROTOBUF_BYTES_FIELD_TAG: u32 = 1 << 3 | 2;
            let field_data_length_varint_size = compute_length_size(someip_message.len());
            // 1 byte for tag + length varint + data
            let message_length = 1 + field_data_length_varint_size + someip_message.len();
            let message_header_varint_size = compute_length_size(message_length);
            if message_header_varint_size + message_length > size {
                return Err(anyhow::Error::msg("SOME/IP data exceeds publication max size"));
            }
            let mut buffer = vec![0; size];
            Self::encode_varint32(message_length as u32, &mut buffer[0..]);
            // fits in 1 byte
            Self::encode_varint32(
                PROTOBUF_BYTES_FIELD_TAG,
                &mut buffer[message_header_varint_size..],
            );
            Self::encode_varint32(
                someip_message.len() as u32,
                &mut buffer[message_header_varint_size + 1..],
            );
            let data_start_index = message_header_varint_size + 1 + field_data_length_varint_size;
            let data_end_index = data_start_index + someip_message.len();
            buffer[data_start_index..data_end_index].copy_from_slice(someip_message);

            if let Some(benchmark) = self.benchmark.as_ref() {
                benchmark.add_measurement(unit_type, time_at_fn_start, Instant::now());
            }
            ProtoFormatData::new(buffer, 0)
        }
    }

    #[tracing::instrument(skip_all, fields(unit_type_name = unit_type.get_unit_type_name()))]
    async fn translate_from_someip_to_protobuf_rpc(
        &self,
        unit_type: &UnitType,
        message_data: &[u8],
        descriptor_id: &SomeIpDescriptorId,
    ) -> anyhow::Result<ProtoFormatData> {
        let time_at_fn_start = Instant::now();

        let rpc_metadata =
            self.descriptor_store.get_rpc_metadata(descriptor_id).ok_or_else(|| {
                anyhow::anyhow!("Could not find metadata for SOME/IP method {:?}", descriptor_id)
            })?;
        let translated_data = self
            .someip_to_protobuf_direct_translator
            .translate_rpc(message_data, rpc_metadata.get_translation_metadata())?;
        if let Some(benchmark) = self.benchmark.as_ref() {
            benchmark.add_measurement(unit_type, time_at_fn_start, Instant::now());
        }
        Ok(translated_data)
    }

    #[tracing::instrument(skip_all, fields(unit_type_name = unit_type.get_unit_type_name()))]
    async fn translate_from_protobuf_to_someip(
        &self,
        service_fqin: &ServiceFqin,
        unit_type: &UnitType,
        message_data: &[u8],
        translation_mode: TranslationMode,
    ) -> anyhow::Result<Vec<u8>> {
        match translation_mode {
            TranslationMode::Topic => {
                let topic_metadata = self
                    .config_provider
                    .get_publication_descriptor(service_fqin, unit_type)
                    .await
                    .ok_or(TranslationError::ConfigProviderError)?;

                let someip_type_mapping =
                    self.get_someip_definition(unit_type, translation_mode)?;
                let someip_message_definition = someip_type_mapping.type_mapping();

                let deserializing_span =
                    tracing::info_span!("deserialize_from_proto_bytes").entered();
                let mut is = protobuf::CodedInputStream::from_bytes(message_data);
                let size = is.read_raw_varint32().map_err(TranslationError::ProtobufError)?.into();
                let _old_limit = is.push_limit(size)?;

                let protobuf_message = topic_metadata.message_descriptor.parse_from(&mut is)?;
                deserializing_span.exit();

                if let Some(someip_message_definition) = someip_message_definition {
                    let _translating_span =
                        tracing::info_span!("translate_protobuf_to_someip_bytes").entered();
                    Ok(someip_serde::serializer::get_someip_bytes_from_message_dyn(
                        self.someip_config,
                        someip_message_definition,
                        &topic_metadata.message_descriptor,
                        &*protobuf_message,
                    )?)
                } else {
                    Ok(Vec::new())
                }
            }
            TranslationMode::RpcRequest(ref method) => {
                let rpc_data = self
                    .config_provider
                    .get_rpc_method_descriptor(service_fqin, unit_type, method)
                    .await
                    .ok_or(TranslationError::ConfigProviderError)?;
                let someip_type_mapping =
                    self.get_someip_definition(unit_type, translation_mode)?;
                let someip_message_definition = someip_type_mapping.type_mapping();

                self.translate_rpc_protobuf_to_someip(
                    message_data,
                    &rpc_data.request_message,
                    someip_message_definition,
                )
            }
            TranslationMode::RpcResponse(ref method) => {
                let rpc_data = self
                    .config_provider
                    .get_rpc_method_descriptor(service_fqin, unit_type, method)
                    .await
                    .ok_or(TranslationError::ConfigProviderError)?;
                let someip_type_mapping =
                    self.get_someip_definition(unit_type, translation_mode)?;
                let someip_message_definition = someip_type_mapping.type_mapping();

                self.translate_rpc_protobuf_to_someip(
                    message_data,
                    &rpc_data.response_message,
                    someip_message_definition,
                )
            }
        }
    }

    async fn get_publication_size_metadata(
        &self,
        service_fqin: &ServiceFqin,
        unit_type: &UnitType,
    ) -> anyhow::Result<(u32, u32)> {
        let topic_metadata = self
            .config_provider
            .get_publication_descriptor(service_fqin, unit_type)
            .await
            .ok_or(TranslationError::ConfigProviderError)?;

        Ok((topic_metadata.size_metadata.message_size, topic_metadata.size_metadata.message_count))
    }
}

#[cfg(test)]
pub mod tests {
    use mockall::predicate::eq;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use std::time::Instant;

    use google_sdv_someip::aidl::google::sdv::someip::{
        SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
    };
    use sdv_comms::id::ServiceFqin;
    use sdv_comms::sd::UnitType;
    use sdv_config_provider::vsidl_provider;
    use sdv_config_provider::vsidl_provider_api::{
        MockVsidlConfigProviderV1, PublicationDescriptor, RpcMethodDescriptor, SizeMetadata,
    };
    use sdv_vsidl_common::proto_size_utils::compute_message_length_including_header;
    use someip_config::broker_params::SdMinorVersionMode;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};
    use someip_serde_reflect::{
        SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
        SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
    };

    use crate::agent::protocol_mapping::ProtocolMapping;
    use crate::agent::protocol_mapping::RpcMethodMappingInfo;
    use crate::agent::protocol_mapping::SomeIpRpcMappingInfo;
    use crate::agent::protocol_mapping::TypeMapping;
    use crate::agent::protocol_mapping::{
        BrokerServiceUnit, MessageTranslationMode, SomeIpEventMappingInfo, SomeIpEventMappingType,
        SomeIpServiceInstanceInfo, SomeIpServiceInstanceMapping, SomeIpTypeMapping,
    };
    use crate::translator::RpcMode;
    use crate::translator::SomeIpDescriptorId;
    use crate::translator::{BrokerTranslator, IBrokerTranslator, TranslationMode};
    use crate::BenchmarkDumpsys;
    use crate::DirectTranslator;
    use crate::SomeIpDescriptorStore;
    use protobuf::MessageField;
    use protobuf::{Message, MessageFull};
    use sdv_core_core_services_unit_someip_broker_test_test_protos::message::*;
    use sdv_core_core_services_unit_someip_broker_test_test_protos::message::{
        ArrayTestMessage, SimpleTestMessage,
    };
    use vsidl_v1::someip::some_ip_mapping::SdvTypeMapping;

    const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::OneByte,
    };

    #[test]
    fn test_encode_varint32_zero() {
        let mut buffer = [0; 5];
        BrokerTranslator::<byteorder::BigEndian>::encode_varint32(0, &mut buffer);
        assert_eq!(&buffer[..1], &[0b00000000]);
    }

    #[test]
    fn test_encode_varint32_one_byte() {
        let mut buffer = [0; 5];
        BrokerTranslator::<byteorder::BigEndian>::encode_varint32(1, &mut buffer);
        assert_eq!(&buffer[..1], &[0b00000001]);

        BrokerTranslator::<byteorder::BigEndian>::encode_varint32(127, &mut buffer);
        assert_eq!(&buffer[..1], &[0b01111111]);
    }

    #[test]
    fn test_encode_varint32_two_bytes() {
        let mut buffer = [0; 5];
        BrokerTranslator::<byteorder::BigEndian>::encode_varint32(128, &mut buffer);
        assert_eq!(&buffer[..2], &[0b10000000, 0b00000001]);

        BrokerTranslator::<byteorder::BigEndian>::encode_varint32(300, &mut buffer);
        assert_eq!(&buffer[..2], &[0b10101100, 0b00000010]);
    }

    #[test]
    fn test_encode_varint32_three_bytes() {
        let mut buffer = [0; 5];
        BrokerTranslator::<byteorder::BigEndian>::encode_varint32(16384, &mut buffer);
        assert_eq!(&buffer[..3], &[0b10000000, 0b10000000, 0b00000001]);
    }

    #[test]
    fn test_encode_varint32_max_value() {
        let mut buffer = [0; 5];
        BrokerTranslator::<byteorder::BigEndian>::encode_varint32(u32::MAX, &mut buffer);
        assert_eq!(&buffer[..5], &[0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b00001111]);
    }

    #[tokio::test]
    async fn translate_from_someip_to_protobuf_topic_fails_dynamic_translation() {
        let message_size = 20;

        let publication_descriptor = PublicationDescriptor {
            message_descriptor: SdvTypeMapping::descriptor(), // any proto descriptor will do
            size_metadata: SizeMetadata {
                message_size,
                message_count: 1,
                field_size_constraints: HashMap::new(),
            },
        };

        let service_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Unit Type should be created");

        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("Unit Type should be created");

        let mut config_provider = MockVsidlConfigProviderV1::new();
        config_provider
            .expect_get_publication_descriptor()
            .with(eq(service_fqin.clone()), eq(unit_type.clone()))
            .times(1)
            .returning(move |_, _| Some(publication_descriptor.clone()));

        let mut type_mapping = TypeMapping::default();

        let someip_type_mapping =
            SomeIpTypeMapping::with_mode(None, MessageTranslationMode::DynamicLibrary);

        type_mapping.topic_type_mapping.insert(unit_type.clone(), someip_type_mapping);

        let input_message = vec![42];

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![SomeIpServiceInstanceMapping {
                someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                    SomeIpService {
                        service_id: 100,
                        instance_id: 1,
                        version: SomeIpServiceVersion { major: 1, minor: 1 },
                    },
                    SdMinorVersionMode::ExactOrAnyMinorVersion,
                ),
                event_infos: vec![SomeIpEventMappingInfo {
                    event_id: 50,
                    eventgroup_id: 60,
                    event_mapping_type: SomeIpEventMappingType::SIMPLE(Box::new(
                        BrokerServiceUnit {
                            service_unit_name: "sample".to_string(),
                            unit_type: unit_type.clone(),
                        },
                    )),
                }],
                rpc_infos: vec![],
            }],
            ..Default::default()
        };

        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &config_provider,
            &service_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );
        let descriptor_id = SomeIpDescriptorId::new_event_id(100, 1, 50);
        let translation_result = broker_translator.translate_from_someip_to_protobuf_topic(
            &unit_type,
            &input_message,
            &descriptor_id,
        );

        assert!(translation_result.is_err());
    }

    #[tokio::test]
    async fn translate_from_someip_to_protobuf_forwards_bytes_correctly() {
        let message_size = 20;
        let publication_size = compute_message_length_including_header(message_size);

        let publication_descriptor = PublicationDescriptor {
            message_descriptor: SdvTypeMapping::descriptor(), // any proto descriptor will do
            size_metadata: SizeMetadata {
                message_size,
                message_count: 1,
                field_size_constraints: HashMap::new(),
            },
        };

        let service_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Unit Type should be created");

        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("Unit Type should be created");

        let mut config_provider = MockVsidlConfigProviderV1::new();
        config_provider
            .expect_get_publication_descriptor()
            .with(eq(service_fqin.clone()), eq(unit_type.clone()))
            .times(1)
            .returning(move |_, _| Some(publication_descriptor.clone()));

        let mut type_mapping = TypeMapping::default();

        let someip_type_mapping =
            SomeIpTypeMapping::with_mode(None, MessageTranslationMode::InterpretAsBytes);

        type_mapping.topic_type_mapping.insert(unit_type.clone(), someip_type_mapping);

        let input_message = vec![42];

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![SomeIpServiceInstanceMapping {
                someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                    SomeIpService {
                        service_id: 100,
                        instance_id: 1,
                        version: SomeIpServiceVersion { major: 1, minor: 1 },
                    },
                    SdMinorVersionMode::ExactOrAnyMinorVersion,
                ),
                event_infos: vec![SomeIpEventMappingInfo {
                    event_id: 50,
                    eventgroup_id: 60,
                    event_mapping_type: SomeIpEventMappingType::SIMPLE(Box::new(
                        BrokerServiceUnit {
                            service_unit_name: "sample".to_string(),
                            unit_type: unit_type.clone(),
                        },
                    )),
                }],
                rpc_infos: vec![],
            }],
            ..Default::default()
        };

        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &config_provider,
            &service_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );

        let descriptor_id = SomeIpDescriptorId::new_event_id(100, 1, 50);
        let translation_result = broker_translator
            .translate_from_someip_to_protobuf_topic(&unit_type, &input_message, &descriptor_id)
            .expect("Translation should succeed");
        let size: usize =
            publication_size.try_into().expect("Publication size should fit in usize");
        // Bytes message size = 3, protobuf tag = 10, field length = 1, data = 42
        let mut expected_result = vec![3, 10, 1, 42];
        expected_result.resize(size, 0);
        assert_eq!(translation_result.get_vec(), expected_result);
    }

    fn get_array_test_message_someip_def() -> SomeIpStructTypeDefinition {
        SomeIpStructTypeDefinition {
            name: "ArrayTestMessageSomeIp".to_string(),
            length_field_size: LenFieldSize::OneByte, // Struct itself has a 1-byte length field
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: "repeated_field".to_string(), // Matches proto field name
                field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                    fixed_length: None,
                    length_field_size: LenFieldSize::OneByte, // Array's length is 1 byte (value = num bytes of elements)
                    array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::I32,
                    )),
                }),
            }],
        }
    }

    // Helper to create common FQIN and UnitType for tests
    fn create_test_fqin_and_unit_type() -> (ServiceFqin, UnitType) {
        let service_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("ServiceFqin should be created");

        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("UnitType should be created");

        (service_fqin, unit_type)
    }

    // --- Test Cases ---

    #[tokio::test]
    async fn test_proto_to_someip_topic_simple_message_zerolen_struct() {
        let proto_msg = SimpleTestMessage {
            a: 1,   // u64
            b: 2.5, // f64
            c: 3,   // u32, but SOME/IP def is U8
            ..Default::default()
        };
        // Proto: SimpleTestMessage { a: 1 (u64), b: 2.5 (f64), c: 3 (u32) }
        // SOME/IP Def: ZeroBytes struct length, fields: a (U64), b (F64), c (U8)
        // Expected: a (8B) + b (8B) + c (1B) = 17 bytes.
        let expected_bytes = vec![
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // a: 1 (u64, Big Endian)
            0x40, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // b: 2.5 (f64, Big Endian)
            0x03, // c: 3 (u8)
        ];
        let someip_struct_def = get_simple_test_message_someip_def();
        let translation_mode = TranslationMode::Topic;

        let (service_fqin, unit_type) = create_test_fqin_and_unit_type();

        let mut config_provider = MockVsidlConfigProviderV1::new();
        let mut type_mapping = TypeMapping::default();

        let someip_type_mapping_instance = SomeIpTypeMapping::with_mode(
            Some(someip_struct_def.clone()),
            MessageTranslationMode::Reflection,
        );

        let publication_descriptor = PublicationDescriptor {
            message_descriptor: SimpleTestMessage::descriptor(),
            size_metadata: SizeMetadata {
                message_size: 1024,
                message_count: 1,
                field_size_constraints: HashMap::new(),
            },
        };
        config_provider
            .expect_get_publication_descriptor()
            .with(eq(service_fqin.clone()), eq(unit_type.clone()))
            .times(1)
            .returning(move |_, _| Some(publication_descriptor.clone()));
        type_mapping
            .topic_type_mapping
            .insert(unit_type.clone(), someip_type_mapping_instance.clone());
        let serialized_proto_bytes = proto_msg.write_length_delimited_to_bytes().unwrap();

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Service FQIN should be created");

        let descriptor_store = SomeIpDescriptorStore::create(
            &ProtocolMapping::default(),
            &type_mapping,
            &config_provider,
            &broker_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );

        let translated_someip_bytes_result = broker_translator
            .translate_from_protobuf_to_someip(
                &service_fqin,
                &unit_type,
                &serialized_proto_bytes,
                translation_mode,
            )
            .await;

        assert!(
            translated_someip_bytes_result.is_ok(),
            "Proto to SOME/IP translation failed: {:?}",
            translated_someip_bytes_result.err()
        );
        assert_eq!(translated_someip_bytes_result.unwrap(), expected_bytes);
    }

    #[tokio::test]
    async fn test_proto_to_someip_topic_array_message_onebyte_len_struct_array() {
        let proto_msg =
            ArrayTestMessage { repeated_field: vec![10i32, -20i32], ..Default::default() };
        let expected_bytes = vec![
            0x09, // Struct length (1B for array len field + 2*4B for elements = 9)
            0x08, // Array data length in bytes (2 elements * 4 bytes/i32 = 8)
            0x00, 0x00, 0x00, 0x0A, // repeated_field[0]: 10 (i32, Big Endian)
            0xFF, 0xFF, 0xFF, 0xEC, // repeated_field[1]: -20 (i32, Big Endian)
        ];
        let someip_struct_def = get_array_test_message_someip_def();
        let translation_mode = TranslationMode::Topic;

        let (service_fqin, unit_type) = create_test_fqin_and_unit_type();

        let mut config_provider = MockVsidlConfigProviderV1::new();
        let mut type_mapping = TypeMapping::default();

        let someip_type_mapping_instance = SomeIpTypeMapping::with_mode(
            Some(someip_struct_def.clone()),
            MessageTranslationMode::Reflection,
        );

        let publication_descriptor = PublicationDescriptor {
            message_descriptor: ArrayTestMessage::descriptor(),
            size_metadata: SizeMetadata {
                message_size: 1024,
                message_count: 1,
                field_size_constraints: HashMap::new(),
            },
        };
        config_provider
            .expect_get_publication_descriptor()
            .with(eq(service_fqin.clone()), eq(unit_type.clone()))
            .times(1)
            .returning(move |_, _| Some(publication_descriptor.clone()));
        type_mapping
            .topic_type_mapping
            .insert(unit_type.clone(), someip_type_mapping_instance.clone());
        let serialized_proto_bytes = proto_msg.write_length_delimited_to_bytes().unwrap();

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Service FQIN should be created");

        let descriptor_store = SomeIpDescriptorStore::create(
            &ProtocolMapping::default(),
            &type_mapping,
            &config_provider,
            &broker_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );

        let translated_someip_bytes_result = broker_translator
            .translate_from_protobuf_to_someip(
                &service_fqin,
                &unit_type,
                &serialized_proto_bytes,
                translation_mode,
            )
            .await;

        assert!(
            translated_someip_bytes_result.is_ok(),
            "Proto to SOME/IP translation failed: {:?}",
            translated_someip_bytes_result.err()
        );
        assert_eq!(translated_someip_bytes_result.unwrap(), expected_bytes);
    }

    #[tokio::test]
    async fn test_proto_to_someip_rpc_request_simple_message_zerolen_struct() {
        let proto_msg = SimpleTestMessage { a: 1, b: 2.5, c: 3, ..Default::default() };
        let expected_bytes = vec![
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // a: 1 (u64, Big Endian)
            0x40, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // b: 2.5 (f64, Big Endian)
            0x03, // c: 3 (u8)
        ];
        let someip_struct_def = get_simple_test_message_someip_def();
        let method_name = "TestMethod".to_string();
        let translation_mode = TranslationMode::RpcRequest(method_name.clone());

        let (service_fqin, unit_type) = create_test_fqin_and_unit_type();

        let mut config_provider = MockVsidlConfigProviderV1::new();
        let mut type_mapping = TypeMapping::default();

        let someip_type_mapping_instance = SomeIpTypeMapping::with_mode(
            Some(someip_struct_def.clone()),
            MessageTranslationMode::Reflection,
        );

        let rpc_method_descriptor = RpcMethodDescriptor {
            request_message: Some(SimpleTestMessage::descriptor()),
            response_message: Some(SimpleTestMessage::descriptor()),
        };
        config_provider
            .expect_get_rpc_method_descriptor()
            .with(eq(service_fqin.clone()), eq(unit_type.clone()), eq(method_name.clone()))
            .times(1)
            .returning(move |_, _, _| Some(rpc_method_descriptor.clone()));

        let mut rpc_method_map = HashMap::new();
        rpc_method_map.insert(
            method_name.clone(),
            (someip_type_mapping_instance.clone(), someip_type_mapping_instance.clone()),
        );
        type_mapping.rpc_type_mapping.insert(unit_type.clone(), rpc_method_map);
        let serialized_proto_bytes = proto_msg.write_to_bytes().unwrap();

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Service FQIN should be created");

        let descriptor_store = SomeIpDescriptorStore::create(
            &ProtocolMapping::default(),
            &type_mapping,
            &config_provider,
            &broker_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );

        let translated_someip_bytes_result = broker_translator
            .translate_from_protobuf_to_someip(
                &service_fqin,
                &unit_type,
                &serialized_proto_bytes,
                translation_mode,
            )
            .await;

        assert!(
            translated_someip_bytes_result.is_ok(),
            "Proto to SOME/IP translation failed: {:?}",
            translated_someip_bytes_result.err()
        );
        assert_eq!(translated_someip_bytes_result.unwrap(), expected_bytes);
    }

    fn create_simple_event_mapping(
        service_unit_name: &str,
        unit_type: UnitType,
    ) -> SomeIpEventMappingType {
        SomeIpEventMappingType::SIMPLE(Box::new(BrokerServiceUnit {
            service_unit_name: service_unit_name.to_string(),
            unit_type,
        }))
    }

    fn create_group_event_mapping(
        unit_type: UnitType,
        field_name_mappings: Vec<HashMap<String, String>>,
        group_event_definition: SomeIpStructTypeDefinition,
    ) -> SomeIpEventMappingType {
        let mut group_mappings = Vec::new();
        for (i, field_name_mapping) in field_name_mappings.iter().enumerate() {
            let broker_service_unit = BrokerServiceUnit {
                service_unit_name: format!("service-unit-{i}"),
                unit_type: unit_type.clone(),
            };
            group_mappings.push((broker_service_unit, field_name_mapping.clone()));
        }
        SomeIpEventMappingType::GROUP(Some(group_event_definition), group_mappings)
    }

    fn create_someip_service_instance(
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

    // This test can be enabled locally to measure performance
    #[tokio::test]
    #[ignore]
    async fn broker_translator_group_event_with_numeric_key_perf_test() {
        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("unknown")
            .sdv_package_name("test.missing")
            .service_bundle_name("NonExistent")
            .service_instance_name("num1")
            .build()
            .expect("Service FQIN should be created");

        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.someip")
            .set_service_bundle_name("SomeIpBroker")
            .set_unit_type_name("MessageOne")
            .build()
            .expect("Unit Type should be created");

        let config_provider = vsidl_provider::new_for_someip_broker_agent();

        const EVENT_PARTS: usize = 12;

        let mut fields = Vec::new();
        let mut field_name_mappings = Vec::new();
        for i in 0..EVENT_PARTS {
            let mut map = HashMap::new();
            fields.push(SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: format!("field{}", i * 3 + 1),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U64,
                ),
            });
            map.insert(format!("field{}", i * 3 + 1), "a".to_string());
            fields.push(SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: format!("field{}", i * 3 + 2),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::F64,
                ),
            });
            map.insert(format!("field{}", i * 3 + 2), "b".to_string());
            fields.push(SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: format!("field{}", i * 3 + 3),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
            });
            map.insert(format!("field{}", i * 3 + 3), "c".to_string());

            field_name_mappings.push(map);
        }

        let group_event_def = SomeIpStructTypeDefinition {
            name: "GroupEvent".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields,
        };

        let group_event_mapping =
            create_group_event_mapping(unit_type.clone(), field_name_mappings, group_event_def);
        let someip_service_instance =
            create_someip_service_instance(100, 1, 0, 200, 300, group_event_mapping.clone());
        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![someip_service_instance],
            ..Default::default()
        };

        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &TypeMapping::default(),
            &config_provider,
            &broker_fqin,
        )
        .await;

        let descriptor_id = SomeIpDescriptorId::new_event_id(100, 1, 200);

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            TypeMapping::default(),
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            Some(Arc::new(BenchmarkDumpsys::new(Duration::from_secs(5)))),
        );

        run_broker_translator_perf_group_event_test(descriptor_id, EVENT_PARTS, broker_translator);
    }

    fn run_broker_translator_perf_test(
        unit_type: UnitType,
        descriptor_id: SomeIpDescriptorId,
        broker_translator: BrokerTranslator<byteorder::BigEndian>,
    ) {
        let mut input_message = Vec::new();

        let u64_value: u64 = 59813;
        let f64_value = 593.54f64;
        let u32_value = 100u32;
        input_message.extend_from_slice(&u64_value.to_be_bytes());
        input_message.extend_from_slice(&f64_value.to_be_bytes());
        input_message.extend_from_slice(&u32_value.to_be_bytes());

        let start_time = Instant::now();
        for _i in 0..10000 {
            // The translator returns the following bytes:
            // [15, 8, 165, 211, 3, 17, 184, 30, 133, 235, 81, 140, 130, 64, 24, 100, 0, 0, 0, 0, 0]
            assert!(broker_translator
                .translate_from_someip_to_protobuf_topic(&unit_type, &input_message, &descriptor_id)
                .is_ok())
        }
        let time_taken = start_time.elapsed();
        assert_eq!(Duration::ZERO, time_taken);
    }

    // This test can be enabled locally to measure performance
    #[tokio::test]
    #[ignore]
    async fn broker_translator_with_numeric_key_perf_test() {
        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("unknown")
            .sdv_package_name("test.missing")
            .service_bundle_name("NonExistent")
            .service_instance_name("num1")
            .build()
            .expect("Service FQIN should be created");

        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.someip")
            .set_service_bundle_name("SomeIpBroker")
            .set_unit_type_name("MessageOne")
            .build()
            .expect("Unit Type should be created");

        let config_provider = vsidl_provider::new_for_someip_broker_agent();

        let simple_event_mapping =
            create_simple_event_mapping("test-service-unit", unit_type.clone());
        let someip_service_instance =
            create_someip_service_instance(100, 1, 0, 200, 300, simple_event_mapping);

        let mut type_mapping = TypeMapping::default();
        let someip_type_mapping = SomeIpTypeMapping::new(Some(SomeIpStructTypeDefinition {
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
        }));
        type_mapping.topic_type_mapping.insert(unit_type.clone(), someip_type_mapping);

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![someip_service_instance],
            ..Default::default()
        };

        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &config_provider,
            &broker_fqin,
        )
        .await;

        let descriptor_id = SomeIpDescriptorId::new_event_id(100, 1, 200);

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            Some(Arc::new(BenchmarkDumpsys::new(Duration::from_secs(5)))),
        );

        run_broker_translator_perf_test(unit_type, descriptor_id, broker_translator);
    }

    fn run_broker_translator_perf_group_event_test(
        descriptor_id: SomeIpDescriptorId,
        event_parts: usize,
        broker_translator: BrokerTranslator<byteorder::BigEndian>,
    ) {
        let mut input_message = Vec::new();

        let u64_value: u64 = 59813;
        let f64_value = 593.54f64;
        let u32_value = 100u32;
        for _i in 0..event_parts {
            input_message.extend_from_slice(&u64_value.to_be_bytes());
            input_message.extend_from_slice(&f64_value.to_be_bytes());
            input_message.extend_from_slice(&u32_value.to_be_bytes());
        }

        let start_time = Instant::now();
        for _i in 0..10000 {
            assert!(broker_translator
                .translate_someip_group_event(&input_message, &descriptor_id)
                .is_ok());
        }
        let time_taken = start_time.elapsed();
        assert_eq!(Duration::ZERO, time_taken);
    }

    #[tokio::test]
    async fn test_translate_someip_group_event() {
        // 1. Setup
        let (service_fqin, unit_type) = create_test_fqin_and_unit_type();
        let event_id = 200;
        let service_id = 100;
        let major_version = 1;

        // --- Create SOME/IP group event definition ---
        let group_event_def = Some(SomeIpStructTypeDefinition {
            name: "GroupEvent".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "field_a".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U32,
                    ),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "field_b".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U16,
                    ),
                },
            ],
        });

        // --- Create mapping for the group event ---
        let group_event_mapping = SomeIpEventMappingType::GROUP(
            group_event_def,
            vec![(
                BrokerServiceUnit {
                    service_unit_name: "part1".to_string(),
                    unit_type: unit_type.clone(),
                },
                HashMap::from([("field_a".to_string(), "c".to_string())]),
            )],
        );
        let service_instance = create_someip_service_instance(
            service_id,
            major_version,
            0,
            event_id,
            300,
            group_event_mapping,
        );

        let sdv_publication_size = 20;

        // --- Mock VsidlConfigProvider to return a descriptor for the event part ---
        let mut config_provider = MockVsidlConfigProviderV1::new();
        let size_metadata = SizeMetadata {
            message_size: sdv_publication_size,
            message_count: 1,
            field_size_constraints: HashMap::new(),
        };

        config_provider.expect_get_publication_descriptor().returning(move |_, _| {
            Some(PublicationDescriptor {
                message_descriptor: SimpleTestMessage::descriptor(),
                size_metadata: size_metadata.clone(),
            })
        });

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![service_instance],
            ..Default::default()
        };

        // --- Create the descriptor store which builds the metadata ---
        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &TypeMapping::default(),
            &config_provider,
            &service_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            TypeMapping::default(),
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );
        // --- Setup similar to direct_translator's test ---
        let mut someip_data = Vec::new();
        someip_data.extend_from_slice(&100u32.to_be_bytes()); // Field 0, used for the event part
        someip_data.extend_from_slice(&42u16.to_be_bytes()); // Field 1

        let descriptor_id = SomeIpDescriptorId::new_event_id(service_id, major_version, event_id);

        // 2. Execute
        let result = broker_translator.translate_someip_group_event(&someip_data, &descriptor_id);

        // 3. Verify
        assert!(result.is_ok());
        let event_parts = result.unwrap();
        assert_eq!(event_parts.len(), 1);

        let proto_data = event_parts[0].as_ref().expect("Translation must succeed");

        let mut expected_data = vec![0; (sdv_publication_size as usize) + 1];
        expected_data[0] = 2;
        expected_data[1] = 24;
        expected_data[2] = 100;
        assert_eq!(proto_data.get_data(), &expected_data);
    }

    async fn test_roundtrip_conversion_rpc_method_descriptor<T: protobuf::MessageFull>(
        input_message: T,
        someip_struct_type_def: SomeIpStructTypeDefinition,
        translation_mode: TranslationMode,
    ) {
        let method_name = match translation_mode.clone() {
            TranslationMode::RpcRequest(method_name) => method_name,
            TranslationMode::RpcResponse(method_name) => method_name,
            TranslationMode::Topic => {
                assert_eq!("TranslationMode::Topic found, should be TranslationMode::RpcRequest or TranslationMode::RpcResponse", "");
                return;
            }
        };

        let rpc_method_descriptor = RpcMethodDescriptor {
            request_message: Some(T::descriptor()),
            response_message: Some(T::descriptor()),
        };

        let service_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Service FQIN should be created");

        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("Unit Type should be created");

        let mut config_provider = MockVsidlConfigProviderV1::new();
        config_provider
            .expect_get_rpc_method_descriptor()
            .with(eq(service_fqin.clone()), eq(unit_type.clone()), eq(method_name.clone()))
            .times(2)
            .returning(move |_, _, _| Some(rpc_method_descriptor.clone()));

        let mut type_mapping = TypeMapping::default();

        let someip_type_mapping = SomeIpTypeMapping::with_mode(
            Some(someip_struct_type_def),
            MessageTranslationMode::Reflection,
        );

        type_mapping.rpc_type_mapping.insert(unit_type.clone(), HashMap::new());
        type_mapping.rpc_type_mapping.get_mut(&unit_type).unwrap().insert(
            method_name.clone(),
            (someip_type_mapping.clone(), someip_type_mapping.clone()),
        );

        let input_message_serialized = input_message.write_to_bytes().unwrap();

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![SomeIpServiceInstanceMapping {
                someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                    SomeIpService {
                        service_id: 100,
                        instance_id: 1,
                        version: SomeIpServiceVersion { major: 1, minor: 1 },
                    },
                    SdMinorVersionMode::ExactOrAnyMinorVersion,
                ),
                event_infos: vec![],
                rpc_infos: vec![SomeIpRpcMappingInfo {
                    broker_service_unit: BrokerServiceUnit {
                        service_unit_name: String::from("su_name"),
                        unit_type: unit_type.clone(),
                    },
                    rpc_method_infos: vec![RpcMethodMappingInfo {
                        sdv_method_name: method_name.clone(),
                        someip_method_id: 4,
                    }],
                }],
            }],
            ..Default::default()
        };

        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &config_provider,
            &service_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );

        let translation_result_to_someip = broker_translator
            .translate_from_protobuf_to_someip(
                &service_fqin,
                &unit_type,
                &input_message_serialized,
                translation_mode.clone(),
            )
            .await
            .expect("Translation should succeed");

        let id = SomeIpDescriptorId::new_method_id(100, 1, 4, RpcMode::Response);

        let translation_result_to_proto = broker_translator
            .translate_from_someip_to_protobuf_rpc(&unit_type, &translation_result_to_someip, &id)
            .await
            .expect("Translation should succeed");

        assert_eq!(
            T::parse_from_bytes(translation_result_to_proto.get_data())
                .expect("Parsing should succeed"),
            input_message
        );
    }

    async fn test_roundtrip_conversion_publication_descriptor<T: protobuf::MessageFull>(
        input_message: T,
        someip_struct_type_def: SomeIpStructTypeDefinition,
    ) {
        let publication_descriptor = PublicationDescriptor {
            message_descriptor: T::descriptor(), // any proto descriptor will do
            size_metadata: SizeMetadata {
                message_size: 1000,
                message_count: 1,
                field_size_constraints: HashMap::new(),
            },
        };

        let service_fqin = ServiceFqin::builder()
            .sdv_vm_name("instance1")
            .sdv_package_name("test.package")
            .service_bundle_name("TestBundle")
            .service_instance_name("num1")
            .build()
            .expect("Unit Type should be created");

        let unit_type = UnitType::builder()
            .set_sdv_package_name("test.package")
            .set_service_bundle_name("TestBundle")
            .set_unit_type_name("TestUnitTypeName")
            .build()
            .expect("Unit Type should be created");

        let mut config_provider = MockVsidlConfigProviderV1::new();
        config_provider
            .expect_get_publication_descriptor()
            .with(eq(service_fqin.clone()), eq(unit_type.clone()))
            .times(2)
            .returning(move |_, _| Some(publication_descriptor.clone()));

        let mut type_mapping = TypeMapping::default();

        let someip_type_mapping = SomeIpTypeMapping::with_mode(
            Some(someip_struct_type_def),
            MessageTranslationMode::Reflection,
        );

        type_mapping.topic_type_mapping.insert(unit_type.clone(), someip_type_mapping);

        let input_message_serialized = input_message.write_length_delimited_to_bytes().unwrap();

        let protocol_mapping = ProtocolMapping {
            someip_service_instances: vec![SomeIpServiceInstanceMapping {
                someip_service_instance_info: SomeIpServiceInstanceInfo::new(
                    SomeIpService {
                        service_id: 100,
                        instance_id: 1,
                        version: SomeIpServiceVersion { major: 1, minor: 1 },
                    },
                    SdMinorVersionMode::ExactOrAnyMinorVersion,
                ),
                event_infos: vec![SomeIpEventMappingInfo {
                    event_id: 50,
                    eventgroup_id: 60,
                    event_mapping_type: SomeIpEventMappingType::SIMPLE(Box::new(
                        BrokerServiceUnit {
                            service_unit_name: String::from("sample"),
                            unit_type: unit_type.clone(),
                        },
                    )),
                }],
                rpc_infos: vec![],
            }],
            ..Default::default()
        };

        let descriptor_store = SomeIpDescriptorStore::create(
            &protocol_mapping,
            &type_mapping,
            &config_provider,
            &service_fqin,
        )
        .await;

        let broker_translator = BrokerTranslator::new(
            Box::new(config_provider),
            descriptor_store,
            type_mapping,
            DEFAULT_SOMEIP_CONFIG,
            DirectTranslator::<byteorder::BigEndian>::new(),
            None,
        );
        let descriptor_id = SomeIpDescriptorId::new_event_id(100, 1, 50);

        let translation_result_to_someip = broker_translator
            .translate_from_protobuf_to_someip(
                &service_fqin,
                &unit_type,
                &input_message_serialized,
                TranslationMode::Topic,
            )
            .await
            .expect("Translation should succeed");

        let translation_result_to_proto = broker_translator
            .translate_from_someip_to_protobuf_topic(
                &unit_type,
                &translation_result_to_someip,
                &descriptor_id,
            )
            .expect("Translation should succeed");

        let mut is = protobuf::CodedInputStream::from_bytes(translation_result_to_proto.get_data());
        let size = is.read_raw_varint32().unwrap().into();
        let _old_limit = is.push_limit(size).unwrap();

        let protobuf_message = T::parse_from(&mut is).expect("Parsing should succeed");

        assert_eq!(protobuf_message, input_message);
    }

    fn get_simple_test_message() -> SimpleTestMessage {
        SimpleTestMessage { a: 4321, b: 32.31, c: 11, ..Default::default() }
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

    #[tokio::test]
    async fn test_roundtrip_conversion_rpc_method_descriptor_request_simple_message() {
        test_roundtrip_conversion_rpc_method_descriptor(
            get_simple_test_message(),
            get_simple_test_message_someip_def(),
            TranslationMode::RpcRequest("test_method".to_owned()),
        )
        .await;
    }

    #[tokio::test]
    async fn test_roundtrip_conversion_rpc_method_descriptor_request_complex_message() {
        test_roundtrip_conversion_rpc_method_descriptor(
            get_complex_test_message(),
            get_complex_test_message_someip_def(),
            TranslationMode::RpcRequest("test_method".to_owned()),
        )
        .await;
    }

    #[tokio::test]
    async fn test_roundtrip_conversion_rpc_method_descriptor_response_simple_message() {
        test_roundtrip_conversion_rpc_method_descriptor(
            get_simple_test_message(),
            get_simple_test_message_someip_def(),
            TranslationMode::RpcResponse("test_method".to_owned()),
        )
        .await;
    }

    #[tokio::test]
    async fn test_roundtrip_conversion_rpc_method_descriptor_response_complex_message() {
        test_roundtrip_conversion_rpc_method_descriptor(
            get_complex_test_message(),
            get_complex_test_message_someip_def(),
            TranslationMode::RpcResponse("test_method".to_owned()),
        )
        .await;
    }

    #[tokio::test]
    async fn test_roundtrip_conversion_publication_descriptor_simple_message() {
        test_roundtrip_conversion_publication_descriptor(
            get_simple_test_message(),
            get_simple_test_message_someip_def(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_roundtrip_conversion_publication_descriptor_complex_message() {
        test_roundtrip_conversion_publication_descriptor(
            get_complex_test_message(),
            get_complex_test_message_someip_def(),
        )
        .await;
    }

    fn get_complex_test_message() -> ComplexTestMessage {
        ComplexTestMessage {
            a: MessageField::some(Data {
                a: MessageField::some(A { a: 100, b: 101, c: 102, ..Default::default() }),
                b: 10,
                c: MessageField::some(C { a: 103, b: vec![104, 105, 106], ..Default::default() }),
                d: 11,
                e: 12,
                f: 13,
                g: 14,
                h: vec![
                    H {
                        a: MessageField::some(HA {
                            a: 3,
                            b: 4,
                            c: 5,
                            d: 6,
                            f: 7,
                            g: vec![
                                HAG { a: 33, b: 34, c: 35, ..Default::default() },
                                HAG { a: 43, b: 44, c: 45, ..Default::default() },
                            ],
                            ..Default::default()
                        }),
                        b: MessageField::some(HB {
                            a: 13,
                            b: 14,
                            c: vec![
                                HBC { a: 33, b: 34, ..Default::default() },
                                HBC { a: 43, b: 44, ..Default::default() },
                            ],
                            ..Default::default()
                        }),
                        c: MessageField::some(HC {
                            a: 64,
                            b: vec![1.1, 1.2],
                            c: vec![1.3, 1.4],
                            d: 1.5,
                            e: 1.6,
                            ..Default::default()
                        }),
                        d: MessageField::some(HD {
                            a: vec![11.1, 11.2],
                            b: vec![11.3, 11.4],
                            ..Default::default()
                        }),
                        e: MessageField::some(HE {
                            a: vec![21.1, 21.2],
                            b: vec![21.3, 21.4],
                            c: vec![21.5, 21.6],
                            d: vec![21.7, 21.8],
                            e: 21.9,
                            f: 22.0,
                            g: 15,
                            ..Default::default()
                        }),
                        f: MessageField::some(HF {
                            a: 16,
                            b: vec![
                                HFB { a: 83, b: 84, ..Default::default() },
                                HFB { a: 93, b: 94, ..Default::default() },
                            ],
                            ..Default::default()
                        }),
                        g: MessageField::some(HG {
                            a: 111,
                            b: vec![-1.1, -1.2],
                            ..Default::default()
                        }),
                        h: MessageField::some(HH { a: 93.0094, ..Default::default() }),
                        ..Default::default()
                    },
                    H {
                        a: MessageField::some(HA {
                            a: 2,
                            b: 3,
                            c: 4,
                            d: 5,
                            f: 6,
                            g: vec![
                                HAG { a: 22, b: 23, c: 24, ..Default::default() },
                                HAG { a: 32, b: 33, c: 34, ..Default::default() },
                            ],
                            ..Default::default()
                        }),
                        b: MessageField::some(HB {
                            a: 2,
                            b: 3,
                            c: vec![
                                HBC { a: 22, b: 23, ..Default::default() },
                                HBC { a: 32, b: 33, ..Default::default() },
                            ],
                            ..Default::default()
                        }),
                        c: MessageField::some(HC {
                            a: 53,
                            b: vec![0.0, 0.1],
                            c: vec![0.2, 0.3],
                            d: 0.4,
                            e: 0.5,
                            ..Default::default()
                        }),
                        d: MessageField::some(HD {
                            a: vec![0.0, 0.1],
                            b: vec![0.2, 0.3],
                            ..Default::default()
                        }),
                        e: MessageField::some(HE {
                            a: vec![10.0, 10.1],
                            b: vec![10.2, 10.3],
                            c: vec![10.4, 10.5],
                            d: vec![10.6, 10.7],
                            e: 10.8,
                            f: 11.9,
                            g: 4,
                            ..Default::default()
                        }),
                        f: MessageField::some(HF {
                            a: 5,
                            b: vec![
                                HFB { a: 72, b: 73, ..Default::default() },
                                HFB { a: 82, b: 83, ..Default::default() },
                            ],
                            ..Default::default()
                        }),
                        g: MessageField::some(HG {
                            a: 0,
                            b: vec![-0.0, -0.1],
                            ..Default::default()
                        }),
                        h: MessageField::some(HH { a: 82.9983, ..Default::default() }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn get_primitive(
        field_name: &str,
        someip_type: SomeIpPrimitiveTypeDefinition,
    ) -> SomeIpStructFieldTypeDefinition {
        SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: field_name.to_string(),
            field_type: SomeIpMessageTypeDefinition::PRIMITIVE(someip_type),
        }
    }

    fn get_primitive_array(
        field_name: &str,
        someip_type: SomeIpPrimitiveTypeDefinition,
    ) -> SomeIpStructFieldTypeDefinition {
        SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: field_name.to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(someip_type)),
            }),
        }
    }

    fn get_complex_test_message_someip_def() -> SomeIpStructTypeDefinition {
        let struct_a = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "a".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("c", SomeIpPrimitiveTypeDefinition::U8),
                ]
                .to_vec(),
            }),
        };

        let struct_c = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "c".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::U8),
                ]
                .to_vec(),
            }),
        };

        let struct_hag = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "g".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            get_primitive("a", SomeIpPrimitiveTypeDefinition::U32),
                            get_primitive("b", SomeIpPrimitiveTypeDefinition::U32),
                            get_primitive("c", SomeIpPrimitiveTypeDefinition::U8),
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_ha = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "a".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("c", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("d", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("f", SomeIpPrimitiveTypeDefinition::U8),
                    struct_hag,
                ]
                .to_vec(),
            }),
        };

        let struct_hbc = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "c".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                            get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_hb = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "b".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,

                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                    struct_hbc,
                ]
                .to_vec(),
            }),
        };

        let struct_hc = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "c".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("c", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("d", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::F32),
                ]
                .to_vec(),
            }),
        };

        let struct_hd = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "d".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive_array("a", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                ]
                .to_vec(),
            }),
        };

        let struct_he = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "e".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive_array("a", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("c", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("d", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("f", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("g", SomeIpPrimitiveTypeDefinition::U8),
                ]
                .to_vec(),
            }),
        };

        let struct_hfb = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "b".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                            get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_hf = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "f".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [get_primitive("a", SomeIpPrimitiveTypeDefinition::U8), struct_hfb]
                    .to_vec(),
            }),
        };

        let struct_hg = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "g".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                ]
                .to_vec(),
            }),
        };

        let struct_hh = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "h".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [get_primitive("a", SomeIpPrimitiveTypeDefinition::F32)].to_vec(),
            }),
        };

        let struct_h = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "h".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::TwoBytes,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            struct_ha, struct_hb, struct_hc, struct_hd, struct_he, struct_hf,
                            struct_hg, struct_hh,
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_data = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "a".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    struct_a,
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                    struct_c,
                    get_primitive("d", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("f", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("g", SomeIpPrimitiveTypeDefinition::U8),
                    struct_h,
                ]
                .to_vec(),
            }),
        };

        SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: [struct_data].to_vec(),
        }
    }
}
