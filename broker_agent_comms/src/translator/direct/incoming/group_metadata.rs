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
use std::borrow::Cow;
use std::collections::HashMap;

use anyhow::bail;
use byteorder::ByteOrder;
use sdv_config_provider::vsidl_provider_api::PublicationDescriptor;
use someip_config::serde_config::ffi::LenFieldSize;
use someip_serde_reflect::{
    SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition, SomeIpStructTypeDefinition,
};

use crate::translator::direct::incoming::reader::{ISomeIpReader, SomeIpReader};
use crate::translator::direct::metadata::SomeIpPublicationMetadata;
use crate::translator::direct::{SomeIpPrimitiveSize, SomeIpSizeType};
#[derive(Debug, PartialEq)]
pub(crate) enum FieldTypeGroupMetadata {
    Primitive(SomeIpPrimitiveTypeDefinition),
    LengthDelimited(SomeIpSizeType),
    Struct(StructGroupMetadata),
}

#[derive(Debug, PartialEq)]
// Metadata about a SOME/IP group event. We only need to store information that allows us to
// compute the buffer index of each field in the event
pub(crate) struct StructGroupMetadata {
    pub(crate) someip_len_field_size: LenFieldSize,
    pub(crate) fields: Vec<FieldTypeGroupMetadata>,
    pub(crate) precomputed_indices: Option<Vec<usize>>,
}

impl StructGroupMetadata {
    pub(crate) fn new(
        someip_len_field_size: LenFieldSize,
        fields: Vec<FieldTypeGroupMetadata>,
    ) -> Self {
        Self { someip_len_field_size, fields, precomputed_indices: None }
    }

    // Attempt to precompute field indices for this event, using only the metadata information.
    // If the first `n` fields are fixed size, then the first `n+1` field indices can be computed
    // from the metadata definition alone.
    pub(crate) fn precompute_indices(&mut self) {
        let mut current_offset = 0;
        let mut indices = Vec::new();
        match self.someip_len_field_size {
            LenFieldSize::OneByte => {
                current_offset += 1;
            }
            LenFieldSize::TwoBytes => {
                current_offset += 2;
            }
            LenFieldSize::FourBytes => {
                current_offset += 4;
            }
            LenFieldSize::ZeroBytes => {}
            _ => {
                // Logic in metadata builder ensures that we only construct struct metadata
                // for the above cases
                unreachable!("Metadata builder should always construct a valid LenFieldSize")
            }
        }
        for field in &self.fields {
            indices.push(current_offset);
            match field {
                FieldTypeGroupMetadata::Primitive(primitive_type_definition) => {
                    current_offset += primitive_type_definition.get_size();
                }
                FieldTypeGroupMetadata::LengthDelimited(someip_size_type) => {
                    match someip_size_type {
                        SomeIpSizeType::FIXED(fixed_size) => {
                            current_offset += fixed_size;
                        }
                        SomeIpSizeType::VARIABLE(_) => {
                            // not a fixed size field, we can't precompute anymore
                            break;
                        }
                    }
                }
                FieldTypeGroupMetadata::Struct(struct_group_metadata) => {
                    match struct_group_metadata.precompute_size() {
                        Some(size) => {
                            current_offset += size;
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }

        // The index of the first field is always fixed, so precomputation is only useful if we
        // managed to precompute at least 2 field indices
        if indices.len() >= 2 {
            self.precomputed_indices = Some(indices);
        }
    }

    // Attempt to precompute the size of struct from metadata definition. Returns Some(size) when
    // precomputation is possible (all fields are fixed size), None when a variable length field
    // is present.
    pub(crate) fn precompute_size(&self) -> Option<usize> {
        let mut size = 0;
        match self.someip_len_field_size {
            LenFieldSize::OneByte => {
                size += 1;
            }
            LenFieldSize::TwoBytes => {
                size += 2;
            }
            LenFieldSize::FourBytes => {
                size += 4;
            }
            LenFieldSize::ZeroBytes => {}
            _ => {
                // Logic in metadata builder ensures that we only construct struct metadata
                // for the above cases
                unreachable!()
            }
        }
        for field in &self.fields {
            match field {
                FieldTypeGroupMetadata::Primitive(primitive_type_definition) => {
                    size += primitive_type_definition.get_size();
                }
                FieldTypeGroupMetadata::LengthDelimited(someip_size_type) => {
                    match someip_size_type {
                        SomeIpSizeType::FIXED(fixed_size) => {
                            size += fixed_size;
                        }
                        SomeIpSizeType::VARIABLE(_) => {
                            // not a fixed size field, we can't precompute size
                            return None;
                        }
                    }
                }
                FieldTypeGroupMetadata::Struct(struct_group_metadata) => {
                    size += struct_group_metadata.precompute_size()?;
                }
            }
        }
        Some(size)
    }

    pub(crate) fn compute_field_indices<'lt, B: ByteOrder>(
        &'lt self,
        someip_reader: &mut SomeIpReader<'_, B>,
    ) -> anyhow::Result<Cow<'lt, [usize]>> {
        let mut field_indices = Vec::new();
        if let Some(indices) = &self.precomputed_indices {
            if indices.len() == self.fields.len() {
                return Ok(Cow::Borrowed(indices));
            } else {
                field_indices.extend_from_slice(indices);
            }
        }
        let last_computed_field = field_indices.last();

        if let Some(last_computed_field_index) = last_computed_field {
            // advance by all bytes up to the index of the last computed field index
            someip_reader.advance_by(*last_computed_field_index);
            // advance by this field's size
            Self::advance_field(&self.fields[field_indices.len() - 1], someip_reader)?;
        } else {
            match self.someip_len_field_size {
                LenFieldSize::OneByte => {
                    let _ = someip_reader.read_u8()?;
                }
                LenFieldSize::TwoBytes => {
                    let _ = someip_reader.read_u16()?;
                }
                LenFieldSize::FourBytes => {
                    let _ = someip_reader.read_u32()?;
                }
                LenFieldSize::ZeroBytes => {}
                _ => {
                    // Logic in metadata builder ensures that we only construct struct metadata
                    // for the above cases
                    unreachable!()
                }
            }
        }

        let mut current_buffer_index = someip_reader.num_read_bytes();
        for field in &self.fields[field_indices.len()..] {
            field_indices.push(current_buffer_index);
            Self::advance_field(field, someip_reader)?;
            current_buffer_index = someip_reader.num_read_bytes();
        }
        Ok(Cow::Owned(field_indices))
    }

    fn advance_field<B: ByteOrder>(
        field: &FieldTypeGroupMetadata,
        someip_reader: &mut SomeIpReader<'_, B>,
    ) -> anyhow::Result<()> {
        match field {
            FieldTypeGroupMetadata::Primitive(primitive_type_definition) => {
                let primitive_type_size = primitive_type_definition.get_size();
                let skipped_bytes = someip_reader.advance_by(primitive_type_size);
                if skipped_bytes != primitive_type_size {
                    bail!("Input ended while parsing primitive field")
                }
            }
            FieldTypeGroupMetadata::LengthDelimited(someip_size_type) => {
                // get_size_from_someip_format may read a length field and advance the reader
                // accordingly
                let field_size = someip_size_type.get_size_from_someip_format(someip_reader)?;
                let skipped_bytes = someip_reader.advance_by(field_size);
                if skipped_bytes != field_size {
                    bail!("Input ended while parsing length-delimited field")
                }
            }
            FieldTypeGroupMetadata::Struct(struct_group_metadata) => {
                struct_group_metadata.advance_struct_field(someip_reader)?;
            }
        }
        Ok(())
    }

    fn advance_struct_field<B: ByteOrder>(
        &self,
        someip_reader: &mut SomeIpReader<'_, B>,
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
        if let Some(size) = optional_size {
            let skipped_bytes = someip_reader.advance_by(size);
            if skipped_bytes != size {
                bail!("Input ended while parsing struct field")
            }
            return Ok(());
        }

        for field in &self.fields {
            Self::advance_field(field, someip_reader)?;
        }
        Ok(())
    }
}

// Metadata for a group SOME/IP event
#[derive(Debug)]
pub(crate) struct SomeIpGroupPublicationMetadata {
    // This describes the structure of the entire SOME/IP event
    group_event_metadata: StructGroupMetadata,
    // This describes the individual parts of the SOME/IP group event. The order of the parts
    // corresponds to the order in which the group part descriptors were supplied
    group_parts_metadata: Vec<SomeIpGroupPublicationPartMetadata>,
}

// Metadata for a single group event part
#[derive(Debug)]
pub(crate) struct SomeIpGroupPublicationPartMetadata {
    // Describes the publication metadata corresponding to this part
    metadata: SomeIpPublicationMetadata,
    // Represents what fields in the overall event message must be read to decode this event part
    fields: Vec<usize>,
}

impl SomeIpGroupPublicationMetadata {
    pub(crate) fn get_group_event_metadata(&self) -> &StructGroupMetadata {
        &self.group_event_metadata
    }

    pub(crate) fn get_group_parts_metadata(&self) -> &[SomeIpGroupPublicationPartMetadata] {
        &self.group_parts_metadata
    }

    pub(crate) fn builder() -> SomeIpGroupPublicationMetadataBuilder {
        SomeIpGroupPublicationMetadataBuilder
    }
}

impl SomeIpGroupPublicationPartMetadata {
    pub(crate) fn get_metadata(&self) -> &SomeIpPublicationMetadata {
        &self.metadata
    }

    pub(crate) fn get_fields(&self) -> &[usize] {
        &self.fields
    }
}

pub(crate) struct SomeIpGroupPublicationMetadataBuilder;

impl SomeIpGroupPublicationMetadataBuilder {
    pub(crate) fn build_group_publication_metadata(
        &self,
        group_event_type_definition: &Option<SomeIpStructTypeDefinition>,
        group_parts_publication_descriptors: &Vec<(
            &PublicationDescriptor,
            &HashMap<String, String>,
        )>,
    ) -> anyhow::Result<SomeIpGroupPublicationMetadata> {
        let Some(event_definition) = group_event_type_definition else {
            bail!("SOME/IP group event has an empty definition")
        };

        // Build metadata for incoming event
        let mut group_event_metadata = Self::build_group_event_struct_metadata(event_definition)?;
        group_event_metadata.precompute_indices();

        // Map from incoming field name to field index in message
        let mut someip_field_name_to_index = HashMap::new();
        event_definition.fields.iter().enumerate().for_each(|(index, field)| {
            someip_field_name_to_index.insert(field.field_name.clone(), index);
        });

        // Create a struct type definition for each group event part
        let mut group_parts_metadata = Vec::new();
        for (group_part_descriptor, field_name_mapping) in group_parts_publication_descriptors {
            let mut someip_field_indices = Vec::new();
            let mut event_part_fields = Vec::new();
            // Create a struct type definition from the overall group message, cloning the encessary fields
            // into `event_part_fields`
            for (someip_field_name, proto_field_name) in *field_name_mapping {
                // Field index in incoming message
                let field_index = *someip_field_name_to_index.get(someip_field_name).ok_or_else(|| anyhow::Error::msg(format!("Could not find field with name {someip_field_name} in SOME/IP group event definition")))?;

                let mut someip_field = event_definition
                    .fields
                    .get(field_index)
                    .ok_or_else(|| {
                        anyhow::Error::msg(format!(
                            "Internal error, field index {field_index} is out of bounds"
                        ))
                    })?
                    .clone();
                someip_field.field_name = proto_field_name.clone();
                event_part_fields.push(someip_field);

                someip_field_indices.push(field_index);
            }
            let struct_type_definition = SomeIpStructTypeDefinition {
                // We don't use the struct name, leave it empty
                name: String::new(),
                // This is a "virtual" definition, this struct does not exist as a standalone message
                // in SOME/IP format, so it has no length field
                length_field_size: LenFieldSize::ZeroBytes,
                // Group event part's fields
                fields: event_part_fields,
            };
            // Build translation metadata for group event part
            let metadata = SomeIpPublicationMetadata::builder()
                .build_publication_metadata_from_someip_definition(
                    group_part_descriptor,
                    &struct_type_definition,
                )?;
            group_parts_metadata
                .push(SomeIpGroupPublicationPartMetadata { metadata, fields: someip_field_indices })
        }
        Ok(SomeIpGroupPublicationMetadata { group_event_metadata, group_parts_metadata })
    }

    fn build_group_event_struct_metadata(
        someip_struct_type_definition: &SomeIpStructTypeDefinition,
    ) -> anyhow::Result<StructGroupMetadata> {
        if someip_struct_type_definition.length_field_size != LenFieldSize::ZeroBytes
            && someip_struct_type_definition.length_field_size != LenFieldSize::OneByte
            && someip_struct_type_definition.length_field_size != LenFieldSize::TwoBytes
            && someip_struct_type_definition.length_field_size != LenFieldSize::FourBytes
        {
            bail!("Invalid length field size for struct size type")
        }
        let mut fields = Vec::new();
        for field in &someip_struct_type_definition.fields {
            match field.field_type {
                SomeIpMessageTypeDefinition::PRIMITIVE(primitive_type_def) => {
                    fields.push(FieldTypeGroupMetadata::Primitive(primitive_type_def))
                }
                SomeIpMessageTypeDefinition::STRING(ref string_type_def) => {
                    let someip_size_type = SomeIpSizeType::new(
                        string_type_def.fixed_length,
                        string_type_def.length_field_size,
                    )?;
                    fields.push(FieldTypeGroupMetadata::LengthDelimited(someip_size_type));
                }
                SomeIpMessageTypeDefinition::ARRAY(ref array_type_def) => {
                    let someip_size_type = SomeIpSizeType::new(
                        array_type_def.fixed_length,
                        array_type_def.length_field_size,
                    )?;
                    fields.push(FieldTypeGroupMetadata::LengthDelimited(someip_size_type));
                }
                SomeIpMessageTypeDefinition::STRUCT(ref struct_type_def) => {
                    let nested_struct_metadata =
                        Self::build_group_event_struct_metadata(struct_type_def)?;
                    fields.push(FieldTypeGroupMetadata::Struct(nested_struct_metadata));
                }
            }
        }
        Ok(StructGroupMetadata::new(someip_struct_type_definition.length_field_size, fields))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::direct::incoming::reader::SomeIpReader;
    use byteorder::{BigEndian, WriteBytesExt};
    use protobuf::descriptor::{
        field_descriptor_proto::{Label, Type},
        DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    };
    use protobuf::reflect::FileDescriptor;
    use sdv_config_provider::vsidl_provider_api::{PublicationDescriptor, SizeMetadata};
    use someip_config::serde_config::ffi::LenFieldSize;
    use someip_serde_reflect::{
        SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition, SomeIpStringTypeDefinition,
    };

    #[test]
    fn test_struct_group_metadata_new() {
        let metadata = StructGroupMetadata::new(LenFieldSize::TwoBytes, vec![]);
        assert_eq!(metadata.someip_len_field_size, LenFieldSize::TwoBytes);
        assert!(metadata.fields.is_empty());
    }

    #[test]
    fn test_precompute_indices_simple_primitives() {
        let mut metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32),
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U16),
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U8),
            ],
        );
        metadata.precompute_indices();

        let someip_data = vec![
            0, 0, 0, 1, // u32
            0, 2, // u16
            3, // u8
        ];
        let mut reader = SomeIpReader::<BigEndian>::new(&someip_data);

        let indices_cow = metadata.compute_field_indices(&mut reader).unwrap();

        assert_eq!(indices_cow.as_ref(), vec![0, 4, 6]);
        assert!(matches!(indices_cow, Cow::Borrowed(_)));
        assert_eq!(reader.num_read_bytes(), 0); // Reader should not be advanced
    }

    #[test]
    fn test_precompute_indices_with_length_field() {
        let mut metadata = StructGroupMetadata::new(
            LenFieldSize::TwoBytes,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32),
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U16),
            ],
        );

        metadata.precompute_indices();

        let mut someip_data = vec![];
        someip_data.write_u16::<BigEndian>(6).unwrap(); // Length field
        someip_data.write_u32::<BigEndian>(1).unwrap();
        someip_data.write_u16::<BigEndian>(2).unwrap();

        let mut reader = SomeIpReader::<BigEndian>::new(&someip_data);
        let indices_cow = metadata.compute_field_indices(&mut reader).unwrap();

        assert_eq!(indices_cow.as_ref(), vec![2, 6]);
        assert!(matches!(indices_cow, Cow::Borrowed(_)));
    }

    #[test]
    fn test_compute_field_indices_length_delimited_fields() {
        let metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldTypeGroupMetadata::LengthDelimited(SomeIpSizeType::VARIABLE(
                    LenFieldSize::OneByte,
                )), // e.g. a string
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32),
                FieldTypeGroupMetadata::LengthDelimited(SomeIpSizeType::FIXED(5)), // e.g. a fixed-size array
            ],
        );

        let mut someip_data = vec![];
        // Field 1: variable length string "hi"
        someip_data.write_u8(2).unwrap(); // length
        someip_data.extend_from_slice(b"hi");
        // Field 2: u32
        someip_data.write_u32::<BigEndian>(123).unwrap();
        // Field 3: fixed size array
        someip_data.extend_from_slice(&[1, 2, 3, 4, 5]);

        let mut reader = SomeIpReader::<BigEndian>::new(&someip_data);

        let indices_cow = metadata.compute_field_indices(&mut reader).unwrap();

        // Indices:
        // 0: start of first field
        // 1: start of second field (after 1 byte len + 2 bytes data)
        // 7: start of third field (after 4 bytes u32)
        assert_eq!(indices_cow.as_ref(), vec![0, 3, 7]);
        assert!(matches!(indices_cow, Cow::Owned(_)));
        // Total read: 1 (len) + 2 (str) + 4 (u32) + 5 (array) = 12
        assert_eq!(reader.num_read_bytes(), 12);
    }

    #[test]
    fn test_compute_field_indices_nested_struct() {
        let inner_metadata = StructGroupMetadata::new(
            LenFieldSize::OneByte,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U16),
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U16),
            ],
        );

        let outer_metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U8),
                FieldTypeGroupMetadata::Struct(inner_metadata),
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U8),
            ],
        );

        let mut someip_data = vec![];
        someip_data.write_u8(10).unwrap(); // outer field 1
                                           // inner struct
        someip_data.write_u8(4).unwrap(); // inner struct length
        someip_data.write_u16::<BigEndian>(20).unwrap(); // inner field 1
        someip_data.write_u16::<BigEndian>(30).unwrap(); // inner field 2
                                                         //
        someip_data.write_u8(40).unwrap(); // outer field 3

        let mut reader = SomeIpReader::<BigEndian>::new(&someip_data);
        let indices_cow = outer_metadata.compute_field_indices(&mut reader).unwrap();

        // Indices:
        // 0: start of outer field 1
        // 1: start of inner struct (after 1 byte u8)
        // 6: start of outer field 3 (after 1 byte for inner struct len + 4 bytes for inner struct data)
        assert_eq!(indices_cow.as_ref(), vec![0, 1, 6]);
        assert!(matches!(indices_cow, Cow::Owned(_)));
        assert_eq!(reader.num_read_bytes(), 7);
    }

    #[test]
    fn test_compute_field_indices_insufficient_data() {
        let metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32),
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32),
            ],
        );

        let someip_data = vec![0, 0, 0, 1]; // Only one u32
        let mut reader = SomeIpReader::<BigEndian>::new(&someip_data);

        let result = metadata.compute_field_indices(&mut reader);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Input ended while parsing primitive field");
    }

    #[test]
    fn test_precompute_size_fixed_size_fields() {
        let metadata = StructGroupMetadata::new(
            LenFieldSize::OneByte, // 1 byte
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32), // 4 bytes
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U16), // 2 bytes
                FieldTypeGroupMetadata::LengthDelimited(SomeIpSizeType::FIXED(10)),    // 10 bytes
            ],
        );
        assert_eq!(metadata.precompute_size(), Some(1 + 4 + 2 + 10));
    }

    #[test]
    fn test_precompute_size_variable_size_field() {
        let metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32),
                FieldTypeGroupMetadata::LengthDelimited(SomeIpSizeType::VARIABLE(
                    LenFieldSize::OneByte,
                )),
            ],
        );
        assert_eq!(metadata.precompute_size(), None);
    }

    #[test]
    fn test_precompute_size_nested_fixed_size_struct() {
        let inner_metadata = StructGroupMetadata::new(
            LenFieldSize::TwoBytes, // 2 bytes
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U64), // 8 bytes
            ],
        );
        let outer_metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U8), // 1 byte
                FieldTypeGroupMetadata::Struct(inner_metadata),
            ],
        );
        // 1 (outer primitive) + (2 (inner len) + 8 (inner primitive)) = 11
        assert_eq!(outer_metadata.precompute_size(), Some(11));
    }

    #[test]
    fn test_precompute_size_nested_variable_size_struct() {
        let inner_metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![FieldTypeGroupMetadata::LengthDelimited(SomeIpSizeType::VARIABLE(
                LenFieldSize::OneByte,
            ))],
        );
        let outer_metadata = StructGroupMetadata::new(
            LenFieldSize::ZeroBytes,
            vec![
                FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U8),
                FieldTypeGroupMetadata::Struct(inner_metadata),
            ],
        );

        // Inner struct has variable size, so outer cannot be precomputed.
        assert_eq!(outer_metadata.precompute_size(), None);
    }

    fn make_field(
        name: &str,
        number: i32,
        type_: Type,
        label: Label,
        type_name: Option<&str>,
    ) -> FieldDescriptorProto {
        let mut field = FieldDescriptorProto::new();
        field.set_name(name.to_string());
        field.set_number(number);
        field.set_type(type_);
        field.set_label(label);
        if let Some(tn) = type_name {
            field.set_type_name(tn.to_string());
        }
        field
    }

    fn file_descriptor_for_test() -> FileDescriptorProto {
        let mut part1_msg = DescriptorProto::new();
        part1_msg.set_name("Part1".to_string());
        part1_msg.field.push(make_field(
            "field_a",
            1,
            Type::TYPE_UINT32,
            Label::LABEL_OPTIONAL,
            None,
        ));

        let mut part2_msg = DescriptorProto::new();
        part2_msg.set_name("Part2".to_string());
        part2_msg.field.push(make_field(
            "field_b",
            1,
            Type::TYPE_STRING,
            Label::LABEL_OPTIONAL,
            None,
        ));

        let mut file_descriptor = FileDescriptorProto::new();
        file_descriptor.set_name("test.proto".to_string());
        file_descriptor.set_package("test".to_string());
        file_descriptor.message_type.push(part1_msg);
        file_descriptor.message_type.push(part2_msg);
        file_descriptor
    }

    fn get_publication_descriptor(
        file_descriptor_proto: &FileDescriptorProto,
        message_name: &str,
    ) -> PublicationDescriptor {
        let file_descriptor = FileDescriptor::new_dynamic(file_descriptor_proto.clone(), &[])
            .expect("Failed to create FileDescriptor");
        let message_descriptor =
            file_descriptor.message_by_package_relative_name(message_name).unwrap();
        PublicationDescriptor { message_descriptor, size_metadata: SizeMetadata::default() }
    }

    #[test]
    fn test_build_group_publication_metadata_success() {
        let group_event_def = Some(SomeIpStructTypeDefinition {
            name: "GroupEvent".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![
                someip_serde_reflect::SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "someip_a".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U32,
                    ),
                },
                someip_serde_reflect::SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "someip_b".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                        length_field_size: LenFieldSize::OneByte,
                        bom: someip_config::serde_config::ffi::StringType::UTF_8,
                        fixed_length: None,
                    }),
                },
            ],
        });

        let file_proto = file_descriptor_for_test();
        let pub_desc1 = get_publication_descriptor(&file_proto, "Part1");
        let pub_desc2 = get_publication_descriptor(&file_proto, "Part2");

        let desc1_map = HashMap::from([("someip_a".to_string(), "field_a".to_string())]);
        let desc2_map = HashMap::from([("someip_b".to_string(), "field_b".to_string())]);
        let group_parts_descriptors = vec![(&pub_desc1, &desc1_map), (&pub_desc2, &desc2_map)];

        let builder = SomeIpGroupPublicationMetadata::builder();
        let result =
            builder.build_group_publication_metadata(&group_event_def, &group_parts_descriptors);

        assert!(result.is_ok());
        let metadata = result.unwrap();

        // Check group_event_metadata
        assert_eq!(metadata.group_event_metadata.someip_len_field_size, LenFieldSize::ZeroBytes);
        assert_eq!(metadata.group_event_metadata.fields.len(), 2);
        assert_eq!(
            metadata.group_event_metadata.fields[0],
            FieldTypeGroupMetadata::Primitive(SomeIpPrimitiveTypeDefinition::U32)
        );
        assert_eq!(
            metadata.group_event_metadata.fields[1],
            FieldTypeGroupMetadata::LengthDelimited(SomeIpSizeType::VARIABLE(
                LenFieldSize::OneByte
            ))
        );

        // Check group_parts_metadata
        assert_eq!(metadata.group_parts_metadata.len(), 2);

        // Part 1
        let part1_meta = &metadata.group_parts_metadata[0];
        assert_eq!(part1_meta.fields, vec![0]); // "someip_a" is at index 0
        let part1_translation_meta = part1_meta.metadata.translation_metadata();
        assert_eq!(part1_translation_meta.fields.len(), 1);
        assert_eq!(part1_translation_meta.fields[0].protobuf_field_number, 1); // "field_a" is number 1

        // Part 2
        let part2_meta = &metadata.group_parts_metadata[1];
        assert_eq!(part2_meta.fields, vec![1]); // "someip_b" is at index 1
        let part2_translation_meta = part2_meta.metadata.translation_metadata();
        assert_eq!(part2_translation_meta.fields.len(), 1);
        assert_eq!(part2_translation_meta.fields[0].protobuf_field_number, 1); // "field_b" is number 1
    }

    #[test]
    fn test_build_group_publication_metadata_empty_def() {
        let builder = SomeIpGroupPublicationMetadata::builder();

        let result = builder.build_group_publication_metadata(&None, &vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_group_publication_metadata_field_not_found() {
        let group_event_def = Some(SomeIpStructTypeDefinition {
            name: "GroupEvent".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![someip_serde_reflect::SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: "someip_a".to_string(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
            }],
        });

        let file_proto = file_descriptor_for_test();
        let pub_desc1 = get_publication_descriptor(&file_proto, "Part1");

        let desc_map = HashMap::from([("non_existent_field".to_string(), "field_a".to_string())]);

        let group_parts_descriptors = vec![(&pub_desc1, &desc_map)];

        let builder = SomeIpGroupPublicationMetadata::builder();
        let result =
            builder.build_group_publication_metadata(&group_event_def, &group_parts_descriptors);
        assert!(result.is_err());
    }
}
