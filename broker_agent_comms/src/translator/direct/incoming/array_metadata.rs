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

use anyhow::bail;
use protobuf::{descriptor::field_descriptor_proto::Type, rt::WireType};
use sdv_wire_format::reflection::codec::api::protobuf::IProtobufWriter;
use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

use crate::translator::direct::{
    incoming::{
        reader::ISomeIpReader, string_metadata::StringTypeMetadata,
        struct_metadata::StructTypeMetadata,
    },
    valid_mappings::{TRANSLATION_OPERATION_INDICES, VALID_PRIMITIVE_TYPE_MAPPINGS},
    SomeIpSizeType,
};

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum ArrayType {
    // Array of primitive values
    Primitive(u8),
    // Array of string values
    String(StringTypeMetadata),
    // Array of bytes. This corresponds to a `bytes` protobuf field and a SOME/IP array of u8
    Bytes,
    // Array of repeated bytes. This corresponds to a `repeated bytes` protobuf field and a SOME/IP 2d array of u8
    RepeatedBytes(SomeIpSizeType),
    // Array of structs. This corresponds to a `repeated M` field in protobuf (where M is a protobuf message) and a SOME/IP array of structs
    Struct(StructTypeMetadata),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ArrayTypeMetadata {
    pub(crate) someip_size_type: SomeIpSizeType,
    pub(crate) array_type: ArrayType,
}

impl ArrayTypeMetadata {
    pub(crate) fn new_primitive(
        someip_size_type: SomeIpSizeType,
        someip_primitive_type_definition: SomeIpPrimitiveTypeDefinition,
        protobuf_field_type: Type,
    ) -> anyhow::Result<Self> {
        let translation_operation_index = Self::get_translate_operation_index(
            someip_primitive_type_definition,
            protobuf_field_type,
        )?;

        Ok(Self { someip_size_type, array_type: ArrayType::Primitive(translation_operation_index) })
    }

    pub(crate) fn new_string(
        someip_size_type: SomeIpSizeType,
        string_type_metadata: StringTypeMetadata,
    ) -> Self {
        Self { someip_size_type, array_type: ArrayType::String(string_type_metadata) }
    }

    pub(crate) fn new_bytes(someip_size_type: SomeIpSizeType) -> Self {
        Self { someip_size_type, array_type: ArrayType::Bytes }
    }

    pub(crate) fn new_repeated_bytes(
        someip_size_type: SomeIpSizeType,
        nested_size_type: SomeIpSizeType,
    ) -> Self {
        Self { someip_size_type, array_type: ArrayType::RepeatedBytes(nested_size_type) }
    }

    pub(crate) fn new_struct(
        someip_size_type: SomeIpSizeType,
        struct_type_metadata: StructTypeMetadata,
    ) -> Self {
        Self { someip_size_type, array_type: ArrayType::Struct(struct_type_metadata) }
    }

    pub(crate) fn translate_data<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        &self,
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let someip_array_byte_size =
            self.someip_size_type.get_size_from_someip_format(someip_reader)?;
        match &self.array_type {
            ArrayType::Primitive(translate_operation_index) => {
                if someip_array_byte_size == 0 {
                    return Ok(());
                }
                protobuf_writer.write_tag(proto_field_number, WireType::LengthDelimited)?;
                Self::translate_primitive_array(
                    *translate_operation_index,
                    someip_array_byte_size,
                    someip_reader,
                    protobuf_writer,
                )
            }
            ArrayType::Bytes => {
                let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;
                Ok(protobuf_writer.write_bytes(proto_field_number, byte_array)?)
            }
            ArrayType::Struct(struct_type_metadata) => {
                let mut read_bytes = 0;
                let start_index = someip_reader.num_read_bytes();
                while read_bytes < someip_array_byte_size {
                    struct_type_metadata.translate_data(
                        someip_reader,
                        proto_field_number,
                        protobuf_writer,
                    )?;
                    read_bytes = someip_reader.num_read_bytes() - start_index;
                }
                if read_bytes != someip_array_byte_size {
                    bail!("Array size exceeded specified length - specified length: {someip_array_byte_size}, read bytes: {read_bytes}")
                }
                Ok(())
            }
            ArrayType::String(string_type_metadata) => {
                let mut read_bytes = 0;
                let start_index = someip_reader.num_read_bytes();
                while read_bytes < someip_array_byte_size {
                    string_type_metadata.translate_data(
                        someip_reader,
                        proto_field_number,
                        protobuf_writer,
                    )?;
                    read_bytes = someip_reader.num_read_bytes() - start_index;
                }
                if read_bytes != someip_array_byte_size {
                    bail!("Array size exceeded specified length - specified length: {someip_array_byte_size}, read bytes: {read_bytes}")
                }
                Ok(())
            }
            ArrayType::RepeatedBytes(someip_nested_array_size) => {
                let mut read_bytes = 0;
                let start_index = someip_reader.num_read_bytes();
                while read_bytes < someip_array_byte_size {
                    let bytes_len =
                        someip_nested_array_size.get_size_from_someip_format(someip_reader)?;
                    protobuf_writer.write_bytes(
                        proto_field_number,
                        someip_reader.read_bytes_array(bytes_len)?,
                    )?;
                    read_bytes = someip_reader.num_read_bytes() - start_index;
                }
                if read_bytes != someip_array_byte_size {
                    bail!("Array size exceeded specified length - specified length: {someip_array_byte_size}, read bytes: {read_bytes}")
                }
                Ok(())
            }
        }
    }

    fn get_translate_operation_index(
        someip_primitive_type: SomeIpPrimitiveTypeDefinition,
        protobuf_field_type: Type,
    ) -> anyhow::Result<u8> {
        if !VALID_PRIMITIVE_TYPE_MAPPINGS
            .get(&someip_primitive_type)
            .is_some_and(|s| s.contains(&protobuf_field_type))
        {
            bail!(
                "Type mismatch: SOME/IP type {:?} cannot be mapped to proto type {:?}",
                someip_primitive_type,
                protobuf_field_type
            );
        }
        TRANSLATION_OPERATION_INDICES
            .get(&(someip_primitive_type, protobuf_field_type))
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No translation operation found for {:?} -> {:?}",
                    someip_primitive_type,
                    protobuf_field_type
                )
            })
    }

    fn translate_primitive_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        translate_operation_index: u8,
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        match translate_operation_index {
            0 => Self::translate_boolean_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            1 => Self::translate_u8_to_uint32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            2 => Self::translate_u8_to_enum_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            3 => Self::translate_u8_to_fixed32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            4 => Self::translate_u8_to_uint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            5 => Self::translate_u8_to_fixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            6 => Self::translate_u16_to_uint32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            7 => Self::translate_u16_to_enum_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            8 => Self::translate_u16_to_fixed32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            9 => Self::translate_u16_to_uint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            10 => Self::translate_u16_to_fixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            11 => Self::translate_u32_to_uint32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            12 => Self::translate_u32_to_fixed32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            13 => Self::translate_u32_to_uint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            14 => Self::translate_u32_to_fixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            15 => Self::translate_u64_to_uint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            16 => Self::translate_u64_to_fixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            17 => Self::translate_i8_to_sint32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            18 => Self::translate_i8_to_int32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            19 => Self::translate_i8_to_enum_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            20 => Self::translate_i8_to_sfixed32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            21 => Self::translate_i8_to_sint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            22 => Self::translate_i8_to_int64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            23 => Self::translate_i8_to_sfixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            24 => Self::translate_i16_to_int32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            25 => Self::translate_i16_to_sint32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            26 => Self::translate_i16_to_enum_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            27 => Self::translate_i16_to_sfixed32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            28 => Self::translate_i16_to_int64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            29 => Self::translate_i16_to_sint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            30 => Self::translate_i16_to_sfixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            31 => Self::translate_i32_to_int32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            32 => Self::translate_i32_to_sint32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            33 => Self::translate_i32_to_enum_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            34 => Self::translate_i32_to_sfixed32_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            35 => Self::translate_i32_to_int64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            36 => Self::translate_i32_to_sint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            37 => Self::translate_i32_to_sfixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            38 => Self::translate_i64_to_int64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            39 => Self::translate_i64_to_sint64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            40 => Self::translate_i64_to_sfixed64_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            41 => Self::translate_f32_to_float_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            42 => Self::translate_f32_to_double_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            43 => Self::translate_f64_to_double_array(
                someip_array_byte_size,
                someip_reader,
                protobuf_writer,
            ),
            _ => bail!("Unknown translation operation index: {}", translate_operation_index),
        }
    }

    fn translate_boolean_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        protobuf_writer.write_raw_varint32(byte_array.len() as u32)?;
        for byte in byte_array {
            protobuf_writer.write_bool_no_tag(*byte != 0)?;
        }
        Ok(())
    }

    fn translate_u8_to_uint32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_uint32_no_tag(*value as u32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u8_to_enum_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_enum_no_tag(*value as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u8_to_fixed32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        let data_size = (byte_array.len() * 4) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for value in byte_array {
            protobuf_writer.write_fixed32_no_tag(*value as u32)?;
        }
        Ok(())
    }

    fn translate_u8_to_uint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_uint64_no_tag(*value as u64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u8_to_fixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        let data_size = (byte_array.len() * 8) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for value in byte_array {
            protobuf_writer.write_fixed64_no_tag(*value as u64)?;
        }
        Ok(())
    }

    fn translate_u16_to_uint32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of u16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_uint32_no_tag(someip_reader.read_u16_unchecked() as u32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u16_to_enum_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of u16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_enum_no_tag(someip_reader.read_u16_unchecked() as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u16_to_fixed32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of u16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        let data_size = (someip_array_byte_size * 2) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for _ in 0..array_length {
            protobuf_writer.write_fixed32_no_tag(someip_reader.read_u16_unchecked() as u32)?;
        }
        Ok(())
    }

    fn translate_u16_to_uint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of u16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_uint64_no_tag(someip_reader.read_u16_unchecked() as u64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u16_to_fixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of u16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        let data_size = (someip_array_byte_size * 4) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for _ in 0..array_length {
            protobuf_writer.write_fixed64_no_tag(someip_reader.read_u16_unchecked() as u64)?;
        }
        Ok(())
    }

    fn translate_u32_to_uint32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of u32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_uint32_no_tag(someip_reader.read_u32_unchecked())?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u32_to_fixed32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of u32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.write_raw_varint32(someip_array_byte_size as u32)?;
        for _ in 0..array_length {
            protobuf_writer.write_fixed32_no_tag(someip_reader.read_u32_unchecked())?;
        }
        Ok(())
    }

    fn translate_u32_to_uint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of u32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_uint64_no_tag(someip_reader.read_u32_unchecked() as u64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u32_to_fixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of u32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        let data_size = (someip_array_byte_size * 2) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for _ in 0..array_length {
            protobuf_writer.write_fixed64_no_tag(someip_reader.read_u32_unchecked() as u64)?;
        }
        Ok(())
    }

    fn translate_u64_to_uint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(8) {
            bail!("Array of u64s length is not a multiple of 8")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 8;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_uint64_no_tag(someip_reader.read_u64_unchecked())?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_u64_to_fixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(8) {
            bail!("Array of u64s length is not a multiple of 8")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 8;

        protobuf_writer.write_raw_varint32(someip_array_byte_size as u32)?;
        for _ in 0..array_length {
            protobuf_writer.write_fixed64_no_tag(someip_reader.read_u64_unchecked())?;
        }
        Ok(())
    }

    fn translate_i8_to_sint32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_sint32_no_tag(*value as i8 as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i8_to_int32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_int32_no_tag(*value as i8 as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i8_to_enum_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_enum_no_tag(*value as i8 as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i8_to_sfixed32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;

        let data_size = (byte_array.len() * 4) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for value in byte_array {
            protobuf_writer.write_sfixed32_no_tag(*value as i8 as i32)?;
        }
        Ok(())
    }

    fn translate_i8_to_sint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;
        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_sint64_no_tag(*value as i8 as i64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i8_to_int64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;
        protobuf_writer.start_length_delimited_message()?;
        for value in byte_array {
            protobuf_writer.write_int64_no_tag(*value as i8 as i64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i8_to_sfixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let byte_array = someip_reader.read_bytes_array(someip_array_byte_size)?;
        let data_size = (byte_array.len() * 8) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for value in byte_array {
            protobuf_writer.write_sfixed64_no_tag(*value as i8 as i64)?;
        }
        Ok(())
    }

    fn translate_i16_to_int32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of i16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_int32_no_tag(someip_reader.read_i16_unchecked() as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i16_to_sint32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of i16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_sint32_no_tag(someip_reader.read_i16_unchecked() as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i16_to_enum_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of i16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_enum_no_tag(someip_reader.read_i16_unchecked() as i32)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i16_to_sfixed32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of i16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        let data_size = (someip_array_byte_size * 2) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for _ in 0..array_length {
            protobuf_writer.write_sfixed32_no_tag(someip_reader.read_i16_unchecked() as i32)?;
        }
        Ok(())
    }

    fn translate_i16_to_int64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of i16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_int64_no_tag(someip_reader.read_i16_unchecked() as i64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i16_to_sint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of i16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_sint64_no_tag(someip_reader.read_i16_unchecked() as i64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i16_to_sfixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(2) {
            bail!("Array of i16s length is not a multiple of 2")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 2;

        let data_size = (someip_array_byte_size * 4) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for _ in 0..array_length {
            protobuf_writer.write_sfixed64_no_tag(someip_reader.read_i16_unchecked() as i64)?;
        }
        Ok(())
    }

    fn translate_i32_to_int32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of i32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_int32_no_tag(someip_reader.read_i32_unchecked())?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i32_to_sint32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of i32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_sint32_no_tag(someip_reader.read_i32_unchecked())?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i32_to_enum_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of i32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_enum_no_tag(someip_reader.read_i32_unchecked())?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i32_to_sfixed32_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of i32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.write_raw_varint32(someip_array_byte_size as u32)?;
        for _ in 0..array_length {
            protobuf_writer.write_sfixed32_no_tag(someip_reader.read_i32_unchecked())?;
        }
        Ok(())
    }

    fn translate_i32_to_int64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of i32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_int64_no_tag(someip_reader.read_i32_unchecked() as i64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i32_to_sint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of i32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_sint64_no_tag(someip_reader.read_i32_unchecked() as i64)?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i32_to_sfixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of i32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        let data_size = (someip_array_byte_size * 2) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for _ in 0..array_length {
            protobuf_writer.write_sfixed64_no_tag(someip_reader.read_i32_unchecked() as i64)?;
        }
        Ok(())
    }

    fn translate_i64_to_int64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(8) {
            bail!("Array of i64s length is not a multiple of 8")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 8;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_int64_no_tag(someip_reader.read_i64_unchecked())?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i64_to_sint64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(8) {
            bail!("Array of i64s length is not a multiple of 8")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 8;

        protobuf_writer.start_length_delimited_message()?;
        for _ in 0..array_length {
            protobuf_writer.write_sint64_no_tag(someip_reader.read_i64_unchecked())?;
        }
        protobuf_writer.end_length_delimited_message()
    }

    fn translate_i64_to_sfixed64_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(8) {
            bail!("Array of i64s length is not a multiple of 8")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 8;

        protobuf_writer.write_raw_varint32(someip_array_byte_size as u32)?;
        for _ in 0..array_length {
            protobuf_writer.write_sfixed64_no_tag(someip_reader.read_i64_unchecked())?;
        }
        Ok(())
    }

    fn translate_f32_to_float_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of f32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        protobuf_writer.write_raw_varint32(someip_array_byte_size as u32)?;
        for _ in 0..array_length {
            protobuf_writer.write_float_no_tag(someip_reader.read_f32_unchecked())?;
        }
        Ok(())
    }

    fn translate_f32_to_double_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(4) {
            bail!("Array of f32s length is not a multiple of 4")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 4;

        let data_size = (someip_array_byte_size * 2) as u32;
        protobuf_writer.write_raw_varint32(data_size)?;
        for _ in 0..array_length {
            protobuf_writer.write_double_no_tag(someip_reader.read_f32_unchecked() as f64)?;
        }
        Ok(())
    }

    fn translate_f64_to_double_array<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_array_byte_size: usize,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        if !someip_array_byte_size.is_multiple_of(8) {
            bail!("Array of f64s length is not a multiple of 8")
        }
        someip_reader.available_for_unchecked_reads(someip_array_byte_size)?;
        let array_length = someip_array_byte_size / 8;

        protobuf_writer.write_raw_varint32(someip_array_byte_size as u32)?;
        for _ in 0..array_length {
            protobuf_writer.write_double_no_tag(someip_reader.read_f64_unchecked())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::direct::incoming::{
        primitive_metadata::PrimitiveTypeMetadata,
        struct_metadata::{FieldMetadata, FieldTypeMetadata},
        SomeIpReader,
    };
    use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
    use protobuf::CodedOutputStream;
    use sdv_wire_format::reflection::codec::api::protobuf::PublicationWriter;
    use someip_config::serde_config::ffi::LenFieldSize;

    fn test_translate_primitive_array<B, F>(
        someip_primitive_type: SomeIpPrimitiveTypeDefinition,
        protobuf_field_type: Type,
        someip_array_data_bytes: &[u8],
        field_number: u32,
        someip_size_type: SomeIpSizeType,
        expected_protobuf_writer_fn: F,
    ) where
        B: ByteOrder + 'static,
        F: FnOnce(&mut PublicationWriter),
    {
        // Setup ArrayTypeMetadata
        let metadata = ArrayTypeMetadata::new_primitive(
            someip_size_type,
            someip_primitive_type,
            protobuf_field_type,
        )
        .unwrap();

        // Prepare SOME/IP input: add the length prefix to the array data if variable size
        let mut someip_input_bytes = Vec::new();
        match someip_size_type {
            SomeIpSizeType::FIXED(size) => {
                assert_eq!(size, someip_array_data_bytes.len());
            }
            SomeIpSizeType::VARIABLE(len_field_size) => match len_field_size {
                LenFieldSize::OneByte => {
                    someip_input_bytes.write_u8(someip_array_data_bytes.len() as u8).unwrap()
                }
                LenFieldSize::TwoBytes => {
                    someip_input_bytes.write_u16::<B>(someip_array_data_bytes.len() as u16).unwrap()
                }
                LenFieldSize::FourBytes => {
                    someip_input_bytes.write_u32::<B>(someip_array_data_bytes.len() as u32).unwrap()
                }
                _ => panic!("Unsupported LenFieldSize for test helper"),
            },
        }
        someip_input_bytes.extend_from_slice(someip_array_data_bytes);

        let mut someip_reader: SomeIpReader<'_, B> = SomeIpReader::<B>::new(&someip_input_bytes);
        let mut actual_buffer = vec![0; 100];
        let mut actual_protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut actual_buffer).unwrap();

        // Execute serialization
        metadata
            .translate_data(&mut someip_reader, field_number, &mut actual_protobuf_writer)
            .unwrap();

        // Prepare expected protobuf output
        let mut expected_buffer = vec![0; 100];
        let mut expected_protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut expected_buffer).unwrap();
        expected_protobuf_writer_fn(&mut expected_protobuf_writer);

        actual_protobuf_writer.finalize().unwrap();
        expected_protobuf_writer.finalize().unwrap();

        // Verify
        assert_eq!(actual_buffer, expected_buffer);
    }

    #[test]
    fn test_translate_boolean_array() {
        let field_number = 1;
        let someip_array_data = vec![0x01, 0x00, 0x01]; // true, false, true
        let someip_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::OneByte);

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::BOOLEAN,
            Type::TYPE_BOOL,
            &someip_array_data,
            field_number,
            someip_size_type,
            |writer| writer.write_repeated_packed_bool(field_number, &[true, false, true]).unwrap(),
        );
    }

    #[test]
    fn test_translate_u8_to_uint32_array() {
        let field_number = 2;
        let someip_array_data = vec![10, 20, 30];
        let someip_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::OneByte);

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_UINT32,
            &someip_array_data,
            field_number,
            someip_size_type,
            |writer| writer.write_repeated_packed_uint32(field_number, &[10, 20, 30]).unwrap(),
        );
    }

    #[test]
    fn test_translate_u16_to_uint32_array() {
        let field_number = 3;
        let mut someip_array_data = Vec::new();
        someip_array_data.write_u16::<BigEndian>(100).unwrap();
        someip_array_data.write_u16::<BigEndian>(200).unwrap();
        let someip_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes);

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::U16,
            Type::TYPE_UINT32,
            &someip_array_data,
            field_number,
            someip_size_type,
            |writer| writer.write_repeated_packed_uint32(field_number, &[100, 200]).unwrap(),
        );
    }

    #[test]
    fn test_translate_i32_to_int32_array() {
        let field_number = 4;
        let mut someip_array_data = Vec::new();
        someip_array_data.write_i32::<BigEndian>(-1000).unwrap();
        someip_array_data.write_i32::<BigEndian>(2000).unwrap();
        let someip_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes);

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::I32,
            Type::TYPE_INT32,
            &someip_array_data,
            field_number,
            someip_size_type,
            |writer| writer.write_repeated_packed_int32(field_number, &[-1000, 2000]).unwrap(),
        );
    }

    #[test]
    fn test_translate_f32_to_float_array() {
        let field_number = 5;
        let mut someip_array_data = Vec::new();
        someip_array_data.write_f32::<BigEndian>(1.23).unwrap();
        someip_array_data.write_f32::<BigEndian>(-4.56).unwrap();
        let someip_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes);

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::F32,
            Type::TYPE_FLOAT,
            &someip_array_data,
            field_number,
            someip_size_type,
            |writer| writer.write_repeated_packed_float(field_number, &[1.23, -4.56]).unwrap(),
        );
    }

    #[test]
    fn test_translate_u64_to_uint64_array() {
        let field_number = 6;
        let mut someip_array_data = Vec::new();
        someip_array_data.write_u64::<BigEndian>(1_000_000_000_000).unwrap();
        someip_array_data.write_u64::<BigEndian>(2_000_000_000_000).unwrap();
        let someip_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes);

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::U64,
            Type::TYPE_UINT64,
            &someip_array_data,
            field_number,
            someip_size_type,
            |writer| {
                writer
                    .write_repeated_packed_uint64(
                        field_number,
                        &[1_000_000_000_000, 2_000_000_000_000],
                    )
                    .unwrap()
            },
        );
    }

    #[test]
    fn test_translate_primitive_array_empty() {
        let field_number = 7;
        let someip_array_data = vec![];
        let someip_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::OneByte);

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_UINT32,
            &someip_array_data,
            field_number,
            someip_size_type,
            |_| {
                // For empty array, no data is written
            },
        );
    }

    #[test]
    fn test_translate_primitive_array_fixed_size() {
        let field_number = 8;
        let someip_array_data = vec![1, 2, 3, 4, 5];
        let someip_size_type = SomeIpSizeType::FIXED(someip_array_data.len());

        test_translate_primitive_array::<BigEndian, _>(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_UINT32,
            &someip_array_data,
            field_number,
            someip_size_type,
            |writer| writer.write_repeated_packed_uint32(field_number, &[1, 2, 3, 4, 5]).unwrap(),
        );
    }

    #[test]
    fn test_new_primitive() {
        let size_type = SomeIpSizeType::VARIABLE(LenFieldSize::OneByte);
        let metadata = ArrayTypeMetadata::new_primitive(
            size_type,
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_UINT32,
        )
        .unwrap();

        assert_eq!(metadata.someip_size_type, size_type);
        assert!(matches!(metadata.array_type, ArrayType::Primitive(_)));
    }

    #[test]
    fn test_new_string() {
        let size_type = SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes);
        let string_metadata = StringTypeMetadata::new(size_type);
        let metadata = ArrayTypeMetadata::new_string(size_type, string_metadata);

        assert_eq!(metadata.someip_size_type, size_type);
        assert_eq!(metadata.array_type, ArrayType::String(string_metadata));
    }

    #[test]
    fn test_new_bytes() {
        let size_type = SomeIpSizeType::FIXED(10);
        let metadata = ArrayTypeMetadata::new_bytes(size_type);

        assert_eq!(metadata.someip_size_type, size_type);
        assert_eq!(metadata.array_type, ArrayType::Bytes);
    }

    #[test]
    fn test_new_repeated_bytes() {
        let size_type = SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes);
        let nested_size_type = SomeIpSizeType::FIXED(10);
        let metadata = ArrayTypeMetadata::new_repeated_bytes(size_type, nested_size_type);

        assert_eq!(metadata.someip_size_type, size_type);
        assert_eq!(metadata.array_type, ArrayType::RepeatedBytes(nested_size_type));
    }

    #[test]
    fn test_new_struct() {
        let size_type = SomeIpSizeType::VARIABLE(LenFieldSize::ZeroBytes);
        let struct_metadata = StructTypeMetadata::new(LenFieldSize::ZeroBytes, vec![]);
        let metadata = ArrayTypeMetadata::new_struct(size_type, struct_metadata.clone());

        assert_eq!(metadata.someip_size_type, size_type);
        assert_eq!(metadata.array_type, ArrayType::Struct(struct_metadata));
    }

    #[test]
    fn test_translate_data_bytes() {
        let field_number = 1;
        let payload = b"hello";
        let size_type = SomeIpSizeType::VARIABLE(LenFieldSize::OneByte);
        let metadata = ArrayTypeMetadata::new_bytes(size_type);

        let mut someip_data = vec![payload.len() as u8];
        someip_data.extend_from_slice(payload);

        let mut someip_reader = SomeIpReader::<BigEndian>::new(&someip_data);
        let mut actual_buffer = vec![0; 50];
        let mut protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut actual_buffer).unwrap();

        metadata.translate_data(&mut someip_reader, field_number, &mut protobuf_writer).unwrap();

        let mut expected_buffer = vec![0; 50];
        let mut expected_writer =
            PublicationWriter::create_publication_writer(1, &mut expected_buffer).unwrap();
        expected_writer.write_bytes(field_number, payload).unwrap();

        protobuf_writer.finalize().unwrap();
        expected_writer.finalize().unwrap();

        assert_eq!(actual_buffer, expected_buffer);
    }

    #[test]
    fn test_translate_data_repeated_bytes() {
        let field_number = 1;
        let payload1 = b"hello";
        let payload2 = b"world";

        let outer_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes);
        let inner_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::OneByte);
        let metadata = ArrayTypeMetadata::new_repeated_bytes(outer_size_type, inner_size_type);

        let mut inner_payload = Vec::new();
        inner_payload.push(payload1.len() as u8);
        inner_payload.extend_from_slice(payload1);
        inner_payload.push(payload2.len() as u8);
        inner_payload.extend_from_slice(payload2);

        let mut someip_data = Vec::new();
        someip_data.write_u16::<BigEndian>(inner_payload.len() as u16).unwrap();
        someip_data.extend_from_slice(&inner_payload);

        let mut someip_reader = SomeIpReader::<BigEndian>::new(&someip_data);
        let mut actual_buffer = vec![0; 100];
        let mut protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut actual_buffer).unwrap();

        metadata.translate_data(&mut someip_reader, field_number, &mut protobuf_writer).unwrap();

        let mut expected_buffer = vec![0; 100];
        let mut expected_writer =
            PublicationWriter::create_publication_writer(1, &mut expected_buffer).unwrap();
        expected_writer.write_bytes(field_number, payload1).unwrap();
        expected_writer.write_bytes(field_number, payload2).unwrap();

        drop(protobuf_writer);
        drop(expected_writer);

        assert_eq!(actual_buffer, expected_buffer);
    }

    #[test]
    fn test_translate_data_string_array() {
        let field_number = 1;
        let s1 = "hello";
        let s2 = "world";

        let outer_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes);
        let inner_string_metadata =
            StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::OneByte));
        let metadata = ArrayTypeMetadata::new_string(outer_size_type, inner_string_metadata);

        let mut inner_payload = Vec::new();
        let mut s1_data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        s1_data.extend_from_slice(s1.as_bytes());
        s1_data.push(0);
        inner_payload.push(s1_data.len() as u8);
        inner_payload.extend_from_slice(&s1_data);

        let mut s2_data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        s2_data.extend_from_slice(s2.as_bytes());
        s2_data.push(0);
        inner_payload.push(s2_data.len() as u8);
        inner_payload.extend_from_slice(&s2_data);

        let mut someip_data = Vec::new();
        someip_data.write_u16::<BigEndian>(inner_payload.len() as u16).unwrap();
        someip_data.extend_from_slice(&inner_payload);

        let mut someip_reader: SomeIpReader<'_, BigEndian> =
            SomeIpReader::<BigEndian>::new(&someip_data);
        let mut actual_buffer = vec![0; 100];
        let mut protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut actual_buffer).unwrap();

        metadata.translate_data(&mut someip_reader, field_number, &mut protobuf_writer).unwrap();

        let mut expected_buffer = vec![0; 100];
        let mut expected_writer =
            PublicationWriter::create_publication_writer(1, &mut expected_buffer).unwrap();
        expected_writer.write_string(field_number, s1).unwrap();
        expected_writer.write_string(field_number, s2).unwrap();

        drop(protobuf_writer);
        drop(expected_writer);

        assert_eq!(actual_buffer, expected_buffer);
    }

    #[test]
    fn test_translate_data_struct_array() {
        let field_number = 1;

        let primitive_u8 =
            PrimitiveTypeMetadata::new(SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT32)
                .unwrap();
        let primitive_u16 =
            PrimitiveTypeMetadata::new(SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_UINT32)
                .unwrap();
        let struct_metadata = StructTypeMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldMetadata::new(1, FieldTypeMetadata::Primitive(primitive_u8)),
                FieldMetadata::new(2, FieldTypeMetadata::Primitive(primitive_u16)),
            ],
        );

        let outer_size_type = SomeIpSizeType::VARIABLE(LenFieldSize::OneByte);
        let metadata = ArrayTypeMetadata::new_struct(outer_size_type, struct_metadata);

        let mut single_struct_payload = Vec::new();
        single_struct_payload.push(42);
        single_struct_payload.write_u16::<BigEndian>(1000).unwrap();

        let mut someip_data = Vec::new();
        let array_len = 20;

        someip_data.push((single_struct_payload.len() * array_len) as u8);
        for _ in 0..array_len {
            someip_data.extend_from_slice(&single_struct_payload);
        }

        let mut someip_reader = SomeIpReader::<BigEndian>::new(&someip_data);
        let mut actual_buffer = vec![0; array_len * 20 + 2];
        let mut protobuf_writer =
            PublicationWriter::create_publication_writer(2, &mut actual_buffer).unwrap();

        let mut expected_raw_buffer = vec![0; array_len * 20 + 2];
        let mut cos = CodedOutputStream::bytes(&mut expected_raw_buffer);
        for _ in 0..array_len {
            cos.write_tag(field_number, WireType::LengthDelimited).unwrap();
            cos.write_raw_varint32(1 + 1 + 1 + 2).unwrap();
            cos.write_uint32(1, 42).unwrap(); // 1 byte tag + 1 byte data
            cos.write_uint32(2, 1000).unwrap(); // 1 byte tag + 2 byte data
        }
        drop(cos);
        let mut expected_buffer = Vec::new();
        // array_len as a varint
        expected_buffer.push(140);
        expected_buffer.push(1);
        expected_buffer.extend_from_slice(&expected_raw_buffer);

        metadata.translate_data(&mut someip_reader, field_number, &mut protobuf_writer).unwrap();
        protobuf_writer.finalize().unwrap();

        expected_buffer.resize(actual_buffer.len(), 0);

        assert_eq!(actual_buffer, expected_buffer);
    }

    #[test]
    fn test_array_translation_logic_is_consistent() {
        // This test ensures consistency between the declared valid mappings, the implemented
        // translation operations, and the dispatching logic that connects them. It verifies
        // that the index in `TRANSLATION_OPERATION_INDICES` for a given type mapping
        // correctly corresponds to the appropriate translation function
        // Ground truth map of (SOME/IP type, Protobuf type) -> translation function.
        struct ArrayTranslatorTable<'i, 'w, A, B>(
            std::marker::PhantomData<&'i u8>,
            std::marker::PhantomData<&'w u8>,
            std::marker::PhantomData<A>,
            std::marker::PhantomData<B>,
        );

        impl<'i, 'w, A: ISomeIpReader<'i>, B: IProtobufWriter<'w>> ArrayTranslatorTable<'i, 'w, A, B> {
            const OPS: [(
                (SomeIpPrimitiveTypeDefinition, Type),
                fn(usize, &mut A, &mut B) -> anyhow::Result<()>,
            ); 44] = [
                (
                    (SomeIpPrimitiveTypeDefinition::BOOLEAN, Type::TYPE_BOOL),
                    ArrayTypeMetadata::translate_boolean_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT32),
                    ArrayTypeMetadata::translate_u8_to_uint32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_ENUM),
                    ArrayTypeMetadata::translate_u8_to_enum_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_FIXED32),
                    ArrayTypeMetadata::translate_u8_to_fixed32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT64),
                    ArrayTypeMetadata::translate_u8_to_uint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_FIXED64),
                    ArrayTypeMetadata::translate_u8_to_fixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_UINT32),
                    ArrayTypeMetadata::translate_u16_to_uint32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_ENUM),
                    ArrayTypeMetadata::translate_u16_to_enum_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_FIXED32),
                    ArrayTypeMetadata::translate_u16_to_fixed32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_UINT64),
                    ArrayTypeMetadata::translate_u16_to_uint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_FIXED64),
                    ArrayTypeMetadata::translate_u16_to_fixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32),
                    ArrayTypeMetadata::translate_u32_to_uint32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_FIXED32),
                    ArrayTypeMetadata::translate_u32_to_fixed32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT64),
                    ArrayTypeMetadata::translate_u32_to_uint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_FIXED64),
                    ArrayTypeMetadata::translate_u32_to_fixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U64, Type::TYPE_UINT64),
                    ArrayTypeMetadata::translate_u64_to_uint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U64, Type::TYPE_FIXED64),
                    ArrayTypeMetadata::translate_u64_to_fixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SINT32),
                    ArrayTypeMetadata::translate_i8_to_sint32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_INT32),
                    ArrayTypeMetadata::translate_i8_to_int32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_ENUM),
                    ArrayTypeMetadata::translate_i8_to_enum_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SFIXED32),
                    ArrayTypeMetadata::translate_i8_to_sfixed32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SINT64),
                    ArrayTypeMetadata::translate_i8_to_sint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_INT64),
                    ArrayTypeMetadata::translate_i8_to_int64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SFIXED64),
                    ArrayTypeMetadata::translate_i8_to_sfixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_INT32),
                    ArrayTypeMetadata::translate_i16_to_int32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SINT32),
                    ArrayTypeMetadata::translate_i16_to_sint32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_ENUM),
                    ArrayTypeMetadata::translate_i16_to_enum_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SFIXED32),
                    ArrayTypeMetadata::translate_i16_to_sfixed32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_INT64),
                    ArrayTypeMetadata::translate_i16_to_int64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SINT64),
                    ArrayTypeMetadata::translate_i16_to_sint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SFIXED64),
                    ArrayTypeMetadata::translate_i16_to_sfixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_INT32),
                    ArrayTypeMetadata::translate_i32_to_int32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SINT32),
                    ArrayTypeMetadata::translate_i32_to_sint32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_ENUM),
                    ArrayTypeMetadata::translate_i32_to_enum_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SFIXED32),
                    ArrayTypeMetadata::translate_i32_to_sfixed32_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_INT64),
                    ArrayTypeMetadata::translate_i32_to_int64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SINT64),
                    ArrayTypeMetadata::translate_i32_to_sint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SFIXED64),
                    ArrayTypeMetadata::translate_i32_to_sfixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_INT64),
                    ArrayTypeMetadata::translate_i64_to_int64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_SINT64),
                    ArrayTypeMetadata::translate_i64_to_sint64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_SFIXED64),
                    ArrayTypeMetadata::translate_i64_to_sfixed64_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::F32, Type::TYPE_FLOAT),
                    ArrayTypeMetadata::translate_f32_to_float_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::F32, Type::TYPE_DOUBLE),
                    ArrayTypeMetadata::translate_f32_to_double_array as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::F64, Type::TYPE_DOUBLE),
                    ArrayTypeMetadata::translate_f64_to_double_array as _,
                ),
            ];
        }

        let mut someip_array_data = Vec::new();
        let array_length = 20;

        for _ in 0..array_length {
            someip_array_data.extend_from_slice(&28924u64.to_be_bytes());
        }

        let array_byte_length = someip_array_data.len();

        let mut someip_data = Vec::new();
        someip_data.push(array_byte_length as u8);
        someip_data.extend_from_slice(&someip_array_data);

        // Check that every created primitive array metadata points to the correct function
        for (_, index) in TRANSLATION_OPERATION_INDICES.iter() {
            let (someip_type, protobuf_type) =
                ArrayTranslatorTable::<SomeIpReader<'_, BigEndian>, PublicationWriter<'_>>::OPS
                    [*index as usize]
                    .0;

            // Check the function pointer is correct by comparing the output of their serializations

            let array_type_metadata = ArrayTypeMetadata::new_primitive(
                SomeIpSizeType::VARIABLE(LenFieldSize::OneByte),
                someip_type,
                protobuf_type,
            )
            .expect("Metadata must be created");
            let ArrayType::Primitive(translation_index) = array_type_metadata.array_type else {
                panic!("Unexpected array type created for SOME/IP type {someip_type:?} and protobuf type {protobuf_type:?}.")
            };

            assert_eq!(translation_index, *index);

            let mut someip_reader_expected = SomeIpReader::<BigEndian>::new(&someip_array_data);
            // 8 is the size of the maximum widening conversion we do (u8 to u64),
            // 10 is the maximum size of a protobuf primitive (varint64)
            let proto_array_len = array_length * 8 * 10 + 2;

            let mut expected_raw_buffer = vec![0; proto_array_len];
            let mut protobuf_writer_expected =
                PublicationWriter::create_publication_writer(2, &mut expected_raw_buffer).unwrap();

            let mut someip_reader_actual = SomeIpReader::<BigEndian>::new(&someip_data);
            let mut actual_buffer = vec![0; proto_array_len];
            let mut protobuf_writer_actual =
                PublicationWriter::create_publication_writer(2, &mut actual_buffer).unwrap();

            let error_message = format!(
                "Serialization must succeed for pairing ({someip_type:?},{protobuf_type:?})"
            );

            ArrayTranslatorTable::<SomeIpReader<'_, BigEndian>, PublicationWriter<'_>>::OPS
                [*index as usize]
                .1(
                array_byte_length, &mut someip_reader_expected, &mut protobuf_writer_expected
            )
            .expect(&error_message);

            array_type_metadata
                .translate_data(&mut someip_reader_actual, 1, &mut protobuf_writer_actual)
                .expect(&error_message);

            drop(protobuf_writer_expected);
            let mut expected_buffer = Vec::new();
            // copy first 2 bytes (reserved for publication length)
            expected_buffer.push(expected_raw_buffer[0]);
            expected_buffer.push(expected_raw_buffer[1]);
            // write the tag
            expected_buffer.push(1 << 3 | 2);
            // copy the rest of the message
            expected_buffer.extend_from_slice(&expected_raw_buffer[2..]);

            drop(protobuf_writer_actual);
            actual_buffer.resize(expected_buffer.len(), 0);

            assert_eq!(expected_buffer, actual_buffer);
        }
    }
}
