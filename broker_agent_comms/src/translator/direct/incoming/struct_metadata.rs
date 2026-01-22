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
use protobuf::rt::WireType;
use sdv_wire_format::reflection::codec::api::protobuf::IProtobufWriter;
use someip_config::serde_config::ffi::LenFieldSize;

use crate::translator::direct::incoming::{
    array_metadata::ArrayTypeMetadata, primitive_metadata::PrimitiveTypeMetadata,
    reader::ISomeIpReader, string_metadata::StringTypeMetadata,
};

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct FieldMetadata {
    pub(crate) protobuf_field_number: u32,
    pub(crate) field_type: FieldTypeMetadata,
}

impl FieldMetadata {
    pub(crate) fn new(protobuf_field_number: u32, field_type: FieldTypeMetadata) -> Self {
        Self { protobuf_field_number, field_type }
    }

    pub(crate) fn translate_data<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        &self,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        match &self.field_type {
            FieldTypeMetadata::Primitive(primitive_type_metadata) => primitive_type_metadata
                .translate_data(someip_reader, self.protobuf_field_number, protobuf_writer),
            FieldTypeMetadata::Array(array_type_metadata) => array_type_metadata.translate_data(
                someip_reader,
                self.protobuf_field_number,
                protobuf_writer,
            ),
            FieldTypeMetadata::String(string_type_metadata) => string_type_metadata.translate_data(
                someip_reader,
                self.protobuf_field_number,
                protobuf_writer,
            ),
            FieldTypeMetadata::Struct(struct_type_metadata) => struct_type_metadata.translate_data(
                someip_reader,
                self.protobuf_field_number,
                protobuf_writer,
            ),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum FieldTypeMetadata {
    Primitive(PrimitiveTypeMetadata),
    String(StringTypeMetadata),
    Array(ArrayTypeMetadata),
    Struct(StructTypeMetadata),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct StructTypeMetadata {
    pub(crate) someip_len_field_size: LenFieldSize,
    pub(crate) fields: Vec<FieldMetadata>,
}

impl StructTypeMetadata {
    pub(crate) fn new(someip_len_field_size: LenFieldSize, fields: Vec<FieldMetadata>) -> Self {
        Self { someip_len_field_size, fields }
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
        protobuf_writer.write_tag(proto_field_number, WireType::LengthDelimited)?;
        self.translate_data_with_length_delimiter(someip_reader, protobuf_writer)
    }

    // This is the entry point of the serialization
    pub(crate) fn translate_data_without_length_delimiter<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        &self,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let optional_size = match self.someip_len_field_size {
            LenFieldSize::ZeroBytes => None,
            LenFieldSize::OneByte => Some(someip_reader.read_u8()? as usize),
            LenFieldSize::TwoBytes => Some(someip_reader.read_u16()? as usize),
            LenFieldSize::FourBytes => Some(someip_reader.read_u32()? as usize),
            _ => {
                // Logic in metadata builder ensures that we only construct struct metadata
                // for the above cases
                unreachable!()
            }
        };
        let initial_read_bytes = someip_reader.num_read_bytes();
        for field in &self.fields {
            field.translate_data(someip_reader, protobuf_writer)?;
        }
        if let Some(size) = optional_size {
            let read_bytes = someip_reader.num_read_bytes() - initial_read_bytes;
            match read_bytes.cmp(&size) {
                std::cmp::Ordering::Equal => {}
                std::cmp::Ordering::Less => {
                    // skip the remaining bytes
                    let _ = someip_reader.advance_by(size - read_bytes);
                }
                std::cmp::Ordering::Greater => {
                    bail!("Malformed data for SOME/IP message: SOME/IP struct serialized length exceeded specified length in header")
                }
            }
        }
        Ok(())
    }

    pub(crate) fn translate_data_with_length_delimiter<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        &self,
        someip_reader: &mut R,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        protobuf_writer.start_length_delimited_message()?;
        self.translate_data_without_length_delimiter(someip_reader, protobuf_writer)?;
        protobuf_writer.end_length_delimited_message()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::direct::incoming::primitive_metadata::PrimitiveTypeMetadata;

    use protobuf::descriptor::field_descriptor_proto::Type;
    use someip_config::serde_config::ffi::LenFieldSize;
    use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

    #[test]
    fn test_new() {
        let len_field_size = LenFieldSize::TwoBytes;
        let primitive_metadata =
            PrimitiveTypeMetadata::new(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32)
                .expect("PrimitiveTypeMetadata creation should succeed");
        let fields = vec![FieldMetadata::new(1, FieldTypeMetadata::Primitive(primitive_metadata))];
        let metadata = StructTypeMetadata::new(len_field_size, fields.clone());

        assert_eq!(metadata.someip_len_field_size, len_field_size);
        assert_eq!(metadata.fields.len(), 1);
        assert_eq!(metadata.fields, fields);
    }

    #[test]
    fn test_field_metadata_new() {
        let field_number = 1;
        let primitive_metadata =
            PrimitiveTypeMetadata::new(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32)
                .expect("PrimitiveTypeMetadata creation should succeed");
        let field_type = FieldTypeMetadata::Primitive(primitive_metadata);
        let metadata = FieldMetadata::new(field_number, field_type.clone());

        assert_eq!(metadata.protobuf_field_number, field_number);
        assert_eq!(metadata.field_type, field_type);
    }
}
