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
use sdv_vsidl_common::proto_size_utils::compute_message_length_including_header;
use sdv_wire_format::reflection::codec::api::protobuf::{PublicationWriter, RpcWriter};

use crate::translator::direct::incoming::group_metadata::SomeIpGroupPublicationMetadata;
use crate::translator::direct::incoming::group_reader::SomeIpGroupReader;
use crate::translator::direct::incoming::proto_format_data::ProtoFormatData;
use crate::translator::direct::incoming::reader::SomeIpReader;
use crate::translator::direct::incoming::struct_metadata::StructTypeMetadata;
use crate::translator::direct::metadata::SomeIpPublicationMetadata;
use crate::translator::direct::ISomeIpReader;

pub(crate) struct DirectTranslator<B: ByteOrder> {
    _marker: std::marker::PhantomData<B>,
}

impl<B: ByteOrder> DirectTranslator<B> {
    pub(crate) fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn translate_rpc(
        &self,
        someip_data: &[u8],
        method_metadata: &StructTypeMetadata,
    ) -> anyhow::Result<ProtoFormatData> {
        let mut someip_reader: SomeIpReader<B> = SomeIpReader::new(someip_data);
        let mut buffer = Vec::new();
        let mut protobuf_writer = RpcWriter::create_rpc_writer(&mut buffer);
        method_metadata
            .translate_data_without_length_delimiter(&mut someip_reader, &mut protobuf_writer)?;

        protobuf_writer.finalize()?;
        ProtoFormatData::new(buffer, 0)
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn translate_publication(
        &self,
        someip_data: &[u8],
        publication_metadata: &SomeIpPublicationMetadata,
    ) -> anyhow::Result<ProtoFormatData> {
        let mut someip_reader: SomeIpReader<B> = SomeIpReader::new(someip_data);
        Self::translate_single_publication_with_reader(publication_metadata, &mut someip_reader)
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn translate_group_publications(
        &self,
        someip_data: &[u8],
        group_publication_metadata: &SomeIpGroupPublicationMetadata,
    ) -> anyhow::Result<Vec<anyhow::Result<ProtoFormatData>>> {
        let mut someip_reader: SomeIpReader<B> = SomeIpReader::new(someip_data);

        let field_indices = group_publication_metadata
            .get_group_event_metadata()
            .compute_field_indices(&mut someip_reader)?;

        let mut translated_parts = Vec::new();
        for event_part in group_publication_metadata.get_group_parts_metadata() {
            match SomeIpGroupReader::<B>::new(someip_data, &field_indices, event_part.get_fields())
            {
                Ok(mut someip_group_reader) => {
                    let translated_part = Self::translate_single_publication_with_reader(
                        event_part.get_metadata(),
                        &mut someip_group_reader,
                    );

                    translated_parts.push(translated_part);
                }
                Err(e) => {
                    translated_parts.push(Err(e));
                }
            }
        }

        Ok(translated_parts)
    }

    fn translate_single_publication_with_reader<'input_lt, R: ISomeIpReader<'input_lt>>(
        publication_metadata: &SomeIpPublicationMetadata,
        someip_reader: &mut R,
    ) -> anyhow::Result<ProtoFormatData> {
        let sdv_publication_size = publication_metadata.sdv_publication_size();

        let max_publication_size =
            compute_message_length_including_header(sdv_publication_size as u32) as usize;
        let header_max_size = max_publication_size - sdv_publication_size;

        // Construct a buffer with max_publication_size + header_max_size capacity. Although we
        // don't need to write more than `max_publication_size`, we may start our message at an
        // index > 0. Since DT expects a contiguous buffer of size exactly `max_publication_size`,
        // we add the maximum shift in bytes to the max_publication_size. At the end, we resize the
        // buffer so that it takes up exactly max_publication_size bytes contiguously from start
        // index of our message
        let mut buffer = vec![0; max_publication_size + header_max_size];

        // Reserve `header_max_size` bytes at the start for the publication's size
        let mut protobuf_writer =
            PublicationWriter::create_publication_writer(header_max_size, &mut buffer)?;
        publication_metadata
            .translation_metadata()
            .translate_data_without_length_delimiter(someip_reader, &mut protobuf_writer)?;

        let start_index = protobuf_writer.finalize()?;

        // Resize the buffer to be the exact size needed by DT
        buffer.resize(max_publication_size + start_index, 0);
        ProtoFormatData::new(buffer, start_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::direct::incoming::{
        primitive_metadata::PrimitiveTypeMetadata,
        struct_metadata::{FieldMetadata, FieldTypeMetadata, StructTypeMetadata},
    };
    use byteorder::BigEndian;
    use protobuf::descriptor::field_descriptor_proto::Type;
    use protobuf::CodedOutputStream;
    use someip_config::serde_config::ffi::LenFieldSize;
    use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

    // Helper to create a simple primitive field metadata
    fn primitive_field(
        number: u32,
        someip_type: SomeIpPrimitiveTypeDefinition,
        proto_type: Type,
    ) -> FieldMetadata {
        FieldMetadata::new(
            number,
            FieldTypeMetadata::Primitive(
                PrimitiveTypeMetadata::new(someip_type, proto_type)
                    .expect("PrimitiveTypeMetadata creation should succeed"),
            ),
        )
    }

    #[test]
    fn test_translate_publication_simple_struct_fast_path() {
        let translator = DirectTranslator::<BigEndian>::new();
        let metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                primitive_field(1, SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32),
                primitive_field(2, SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32),
            ],
        );

        let someip_data: &[u8] = &[0, 0, 0, 1, 0, 0, 0, 2];

        let sdv_publication_size = 12; // 2 * 5 bytes max size for uint32s + 1 * 2 bytes for field tags

        let someip_publication = SomeIpPublicationMetadata::new(metadata, sdv_publication_size);

        let result = translator.translate_publication(someip_data, &someip_publication).unwrap();

        // Expected serialized length is 13 = 12 bytes for data + 1 for length delimiter
        let expected_serialized_length = 13;
        assert_eq!(result.get_data().len(), expected_serialized_length);

        // Expected buffer: length of message (4), field1(0x08, 0x01), field2(0x10, 0x02)
        let expected_data: &[u8] = &[4, 8, 1, 16, 2, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(result.get_data(), expected_data);
    }

    #[test]
    fn test_translate_publication_with_small_nested_struct_fast_path() {
        let translator = DirectTranslator::<BigEndian>::new();

        let inner_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![primitive_field(1, SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32)],
        );

        let outer_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![FieldMetadata::new(1, FieldTypeMetadata::Struct(inner_metadata))],
        );

        let someip_data: &[u8] = &[0, 0, 0, 42]; // u32 for the inner struct's field

        // Inner message length = 1 byte for field tag + 1 byte for length delimiter + 5 bytes for data = 7 bytes
        let sdv_publication_size = 7;

        let someip_publication =
            SomeIpPublicationMetadata::new(outer_metadata, sdv_publication_size);

        let result = translator.translate_publication(someip_data, &someip_publication).unwrap();

        // Expected serialized length is 8 = 7 bytes for inner message + 1 byte for length delimiter
        let expected_serialized_length = 8;
        assert_eq!(result.get_data().len(), expected_serialized_length);

        // Expected buffer: len(4), tag(0x0A), len(2), data(0x08, 0x2A)
        let expected_data: &[u8] = &[4, 10, 2, 8, 42, 0, 0, 0];
        assert_eq!(result.get_data(), expected_data);
    }

    #[test]
    fn test_translate_publication_with_large_nested_struct_fixed_data() {
        const NUM_FIELDS: u32 = 30;
        let translator = DirectTranslator::<BigEndian>::new();

        // Create an inner struct that will serialize to > 127 bytes
        let mut inner_fields = Vec::new();
        for i in 1..=NUM_FIELDS {
            inner_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_FIXED32,
            ));
        }
        let inner_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_fields);

        let outer_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![FieldMetadata::new(1, FieldTypeMetadata::Struct(inner_metadata))],
        );

        // NUM_FIELDS * 4 bytes for the u32 values
        let mut someip_payload = Vec::with_capacity(NUM_FIELDS as usize * 4);
        for i in 1..=NUM_FIELDS {
            someip_payload.extend_from_slice(&i.to_be_bytes());
        }

        // Calculate expected protobuf serialization
        let mut expected_inner_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_payload);
            for i in 1..=NUM_FIELDS {
                cos.write_fixed32(i, i).unwrap();
            }
            cos.flush().unwrap();
        }
        // Fields numbered 1 to 15 will take up 1 byte for the field tag
        // Fields numbered 16 to 30 will take up 2 bytes for the field tag
        // fixed32 types are always encoded using 4 bytes
        // 15*(1-byte-tag+4) + 15*(2-byte-tag+4) = 75 + 90 = 165
        assert_eq!(expected_inner_payload.len(), 165);

        let mut expected_outer_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_outer_payload);
            // this works because bytes and embedded messages have the same wire type
            cos.write_bytes(1, &expected_inner_payload).unwrap();
            cos.flush().unwrap();
        }
        // 1 byte for the tag + 2 bytes for the length field + 165 bytes for data = 168
        assert_eq!(expected_outer_payload.len(), 168);

        let mut expected_final_data = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_final_data);
            cos.write_raw_varint32(expected_outer_payload.len() as u32).unwrap();
            cos.write_raw_bytes(&expected_outer_payload).unwrap();
            cos.flush().unwrap();
        }

        let sdv_publication_size = expected_outer_payload.len();

        let someip_publication =
            SomeIpPublicationMetadata::new(outer_metadata, sdv_publication_size);

        let result = translator
            .translate_publication(&someip_payload, &someip_publication)
            .expect("Translation should succeed");

        assert_eq!(result.get_data(), &expected_final_data);
    }

    #[test]
    fn test_translate_publication_with_large_nested_struct_varint_data() {
        const NUM_FIELDS: u32 = 50;
        let translator = DirectTranslator::<BigEndian>::new();

        // Create an inner struct that will serialize to > 127 bytes
        let mut inner_fields = Vec::new();
        for i in 1..=NUM_FIELDS {
            inner_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_UINT32,
            ));
        }
        let inner_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_fields);

        let outer_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![FieldMetadata::new(1, FieldTypeMetadata::Struct(inner_metadata))],
        );

        // NUM_FIELDS * 4 bytes for the u32 values
        const BASE_VALUE: u32 = 200;
        let mut someip_payload = Vec::with_capacity(NUM_FIELDS as usize * 4);
        for i in BASE_VALUE..=(BASE_VALUE + NUM_FIELDS) {
            someip_payload.extend_from_slice(&i.to_be_bytes());
        }

        // Calculate expected protobuf serialization
        let mut expected_inner_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_payload);
            for i in 1..=NUM_FIELDS {
                cos.write_uint32(i, i + BASE_VALUE - 1).unwrap();
            }
            cos.flush().unwrap();
        }
        // Fields numbered 1 to 15 will take up 1 byte for the field tag
        // Fields numbered 16 to 50 will take up 2 bytes for the field tag
        // Values from 200 to 249 will take up 2 bytes for the value as a varint
        // 15*(1-byte-tag+2) + 35*(2-byte-tag+2) = 45 + 140 = 185
        assert_eq!(expected_inner_payload.len(), 185);

        let mut expected_outer_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_outer_payload);
            // this works because bytes and embedded messages have the same wire type
            cos.write_bytes(1, &expected_inner_payload).unwrap();
            cos.flush().unwrap();
        }
        // 1 byte for the tag + 2 bytes for the length field + 185 bytes for data = 188
        assert_eq!(expected_outer_payload.len(), 188);

        let mut expected_final_data = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_final_data);
            cos.write_raw_varint32(expected_outer_payload.len() as u32).unwrap();
            cos.write_raw_bytes(&expected_outer_payload).unwrap();
            cos.flush().unwrap();
        }

        let sdv_publication_size = expected_outer_payload.len();
        let someip_publication =
            SomeIpPublicationMetadata::new(outer_metadata, sdv_publication_size);

        let result = translator
            .translate_publication(&someip_payload, &someip_publication)
            .expect("Translation should succeed");

        assert_eq!(result.get_data(), &expected_final_data);
    }

    #[test]
    fn test_translate_publication_complex_struct() {
        let translator = DirectTranslator::<BigEndian>::new();

        // --- Metadata Setup ---
        // We are describing a message that looks like the following:
        // message Outer {
        //   InnerA inner_a = 1;
        //   uint32 primitive = 2;
        //   InnerB inner_b = 3;
        // }
        //
        // message InnerA {
        //  fixed32 field1 = 1;
        //  ...
        //  fixed32 field30 = 30;
        // }
        // message InnerB {
        //  fixed32 field1 = 1;
        //  ..
        //  fixed32 field40 = 40;
        //  InnerC inner_c = 41;
        // }
        // message InnerC {
        //  fixed32 field1 = 1;
        //  ..
        //  fixed32 field40 = 40;
        // }

        let mut inner_a_fields = Vec::new();
        for i in 1..=30 {
            inner_a_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_FIXED32,
            ));
        }
        let inner_a_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_a_fields);

        let mut inner_b_fields = Vec::new();
        for i in 1..=40 {
            inner_b_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_FIXED32,
            ));
        }

        let mut inner_c_fields = Vec::new();
        for i in 1..=40 {
            inner_c_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_FIXED32,
            ));
        }

        let inner_c_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_c_fields);

        inner_b_fields.push(FieldMetadata::new(41, FieldTypeMetadata::Struct(inner_c_metadata)));
        let inner_b_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_b_fields);

        let outer_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldMetadata::new(1, FieldTypeMetadata::Struct(inner_a_metadata)),
                primitive_field(2, SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT32),
                FieldMetadata::new(3, FieldTypeMetadata::Struct(inner_b_metadata)),
            ],
        );

        // --- SOME/IP Payload Setup ---
        let mut someip_payload = Vec::new();
        for i in 1..=30 {
            someip_payload.extend_from_slice(&(i as u32).to_be_bytes());
        }
        // primitive field in outer message
        someip_payload.push(123);
        // first 40 fields in inner b
        for i in 1..=40 {
            someip_payload.extend_from_slice(&(i as u32).to_be_bytes());
        }
        // inner_c fields
        for i in 1..=40 {
            someip_payload.extend_from_slice(&(i as u32).to_be_bytes());
        }

        // --- Expected Protobuf Payload Calculation ---
        let mut expected_inner_a_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_a_payload);
            for i in 1..=30 {
                cos.write_fixed32(i, i).unwrap();
            }
            cos.flush().unwrap();
        }

        let mut expected_inner_c_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_c_payload);
            for i in 1..=40 {
                cos.write_fixed32(i, i).unwrap();
            }
            cos.flush().unwrap();
        }

        let mut expected_inner_b_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_b_payload);
            for i in 1..=40 {
                cos.write_fixed32(i, i).unwrap();
            }
            cos.write_bytes(41, &expected_inner_c_payload).unwrap();
            cos.flush().unwrap();
        }

        let mut expected_outer_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_outer_payload);
            cos.write_bytes(1, &expected_inner_a_payload).unwrap();
            cos.write_uint32(2, 123).unwrap();
            cos.write_bytes(3, &expected_inner_b_payload).unwrap();
            cos.flush().unwrap();
        }
        let outer_payload_len = expected_outer_payload.len();

        let mut expected_final_data = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_final_data);
            cos.write_raw_varint32(outer_payload_len as u32).unwrap();
            cos.write_raw_bytes(&expected_outer_payload).unwrap();
            cos.flush().unwrap();
        }

        let sdv_publication_size = outer_payload_len;
        let someip_publication =
            SomeIpPublicationMetadata::new(outer_metadata, sdv_publication_size);

        // --- Run Translation ---
        let result =
            translator.translate_publication(&someip_payload, &someip_publication).unwrap();

        // --- Assert ---
        assert_eq!(result.get_data(), &expected_final_data);
    }

    #[test]
    fn test_translate_rpc_simple_struct_fast_path() {
        let translator = DirectTranslator::<BigEndian>::new();
        let metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                primitive_field(1, SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32),
                primitive_field(2, SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32),
            ],
        );

        let someip_data: &[u8] = &[0, 0, 0, 1, 0, 0, 0, 2];

        let result = translator.translate_rpc(someip_data, &metadata).unwrap();

        // Expected buffer: field1(0x08, 0x01), field2(0x10, 0x02)
        let expected_data = vec![8, 1, 16, 2];

        assert_eq!(result.get_vec(), expected_data);
    }

    #[test]
    fn test_translate_rpc_with_small_nested_struct_fast_path() {
        let translator = DirectTranslator::<BigEndian>::new();

        let inner_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![primitive_field(1, SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32)],
        );

        let outer_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![FieldMetadata::new(1, FieldTypeMetadata::Struct(inner_metadata))],
        );

        let someip_data: &[u8] = &[0, 0, 0, 42]; // u32 for the inner struct's field

        let result = translator.translate_rpc(someip_data, &outer_metadata).unwrap();

        // Expected buffer: tag(0x0A), len(2), data(0x08, 0x2A)
        let expected_data = vec![10, 2, 8, 42];
        assert_eq!(result.get_vec(), expected_data);
    }

    #[test]
    fn test_translate_rpc_with_large_nested_struct_varint_data() {
        const NUM_FIELDS: u32 = 50;
        let translator = DirectTranslator::<BigEndian>::new();

        // Create an inner struct that will serialize to > 127 bytes
        let mut inner_fields = Vec::new();
        for i in 1..=NUM_FIELDS {
            inner_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_UINT32,
            ));
        }
        let inner_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_fields);

        let outer_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![FieldMetadata::new(1, FieldTypeMetadata::Struct(inner_metadata))],
        );

        // NUM_FIELDS * 4 bytes for the u32 values
        const BASE_VALUE: u32 = 200;
        let mut someip_payload = Vec::with_capacity(NUM_FIELDS as usize * 4);
        for i in BASE_VALUE..=(BASE_VALUE + NUM_FIELDS) {
            someip_payload.extend_from_slice(&i.to_be_bytes());
        }

        // Calculate expected protobuf serialization
        let mut expected_inner_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_payload);
            for i in 1..=NUM_FIELDS {
                cos.write_uint32(i, i + BASE_VALUE - 1).unwrap();
            }
            cos.flush().unwrap();
        }

        let mut expected_outer_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_outer_payload);
            // this works because bytes and embedded messages have the same wire type
            cos.write_bytes(1, &expected_inner_payload).unwrap();
            cos.flush().unwrap();
        }

        let result = translator
            .translate_rpc(&someip_payload, &outer_metadata)
            .expect("Translation should succeed");

        assert_eq!(result.get_vec(), expected_outer_payload);
    }

    #[test]
    fn test_translate_rpc_complex_struct() {
        let translator = DirectTranslator::<BigEndian>::new();

        // --- Metadata Setup ---
        // We are describing a message that looks like the following:
        // message Outer {
        //   InnerA inner_a = 1;
        //   uint32 primitive = 2;
        //   InnerB inner_b = 3;
        // }
        //
        // message InnerA {
        //  fixed32 field1 = 1;
        //  ...
        //  fixed32 field30 = 30;
        // }
        // message InnerB {
        //  fixed32 field1 = 1;
        //  ..
        //  fixed32 field40 = 40;
        //  InnerC inner_c = 41;
        // }
        // message InnerC {
        //  fixed32 field1 = 1;
        //  ..
        //  fixed32 field40 = 40;
        // }

        let mut inner_a_fields = Vec::new();
        for i in 1..=30 {
            inner_a_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_FIXED32,
            ));
        }
        let inner_a_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_a_fields);

        let mut inner_b_fields = Vec::new();
        for i in 1..=40 {
            inner_b_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_FIXED32,
            ));
        }

        let mut inner_c_fields = Vec::new();
        for i in 1..=40 {
            inner_c_fields.push(primitive_field(
                i,
                SomeIpPrimitiveTypeDefinition::U32,
                Type::TYPE_FIXED32,
            ));
        }

        let inner_c_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_c_fields);

        inner_b_fields.push(FieldMetadata::new(41, FieldTypeMetadata::Struct(inner_c_metadata)));
        let inner_b_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, inner_b_fields);

        let outer_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldMetadata::new(1, FieldTypeMetadata::Struct(inner_a_metadata)),
                primitive_field(2, SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT32),
                FieldMetadata::new(3, FieldTypeMetadata::Struct(inner_b_metadata)),
            ],
        );

        // --- SOME/IP Payload Setup ---
        let mut someip_payload = Vec::new();
        for i in 1..=30 {
            someip_payload.extend_from_slice(&(i as u32).to_be_bytes());
        }
        // primitive field in outer message
        someip_payload.push(123);
        // first 40 fields in inner b
        for i in 1..=40 {
            someip_payload.extend_from_slice(&(i as u32).to_be_bytes());
        }
        // inner_c fields
        for i in 1..=40 {
            someip_payload.extend_from_slice(&(i as u32).to_be_bytes());
        }

        // --- Expected Protobuf Payload Calculation ---
        let mut expected_inner_a_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_a_payload);
            for i in 1..=30 {
                cos.write_fixed32(i, i).unwrap();
            }
            cos.flush().unwrap();
        }

        let mut expected_inner_c_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_c_payload);
            for i in 1..=40 {
                cos.write_fixed32(i, i).unwrap();
            }
            cos.flush().unwrap();
        }

        let mut expected_inner_b_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_inner_b_payload);
            for i in 1..=40 {
                cos.write_fixed32(i, i).unwrap();
            }
            cos.write_bytes(41, &expected_inner_c_payload).unwrap();
            cos.flush().unwrap();
        }

        let mut expected_outer_payload = Vec::new();
        {
            let mut cos = CodedOutputStream::vec(&mut expected_outer_payload);
            cos.write_bytes(1, &expected_inner_a_payload).unwrap();
            cos.write_uint32(2, 123).unwrap();
            cos.write_bytes(3, &expected_inner_b_payload).unwrap();
            cos.flush().unwrap();
        }

        // --- Run Translation ---
        let result = translator.translate_rpc(&someip_payload, &outer_metadata).unwrap();

        // --- Assert ---
        assert_eq!(result.get_vec(), expected_outer_payload);
    }
}
