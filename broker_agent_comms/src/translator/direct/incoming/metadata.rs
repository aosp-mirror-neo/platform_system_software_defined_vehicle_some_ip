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
use protobuf::reflect::{FieldDescriptor, RuntimeFieldType, RuntimeType};
use protobuf::{descriptor::field_descriptor_proto::Type, reflect::MessageDescriptor};
use sdv_config_provider::vsidl_provider_api::PublicationDescriptor;
use someip_config::serde_config::ffi::LenFieldSize;
use someip_serde_reflect::{
    SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
    SomeIpStringTypeDefinition, SomeIpStructTypeDefinition,
};

use crate::agent::protocol_mapping::SomeIpTypeMapping;
use crate::translator::direct::incoming::primitive_metadata::PrimitiveTypeMetadata;
use crate::translator::direct::{
    incoming::{
        array_metadata::ArrayTypeMetadata,
        string_metadata::StringTypeMetadata,
        struct_metadata::{FieldMetadata, FieldTypeMetadata, StructTypeMetadata},
    },
    SomeIpSizeType,
};

#[derive(Debug)]
pub(crate) struct SomeIpPublicationMetadata {
    translation_metadata: StructTypeMetadata,
    sdv_publication_size: usize,
}

impl SomeIpPublicationMetadata {
    pub(crate) fn new(
        translation_metadata: StructTypeMetadata,
        sdv_publication_size: usize,
    ) -> SomeIpPublicationMetadata {
        SomeIpPublicationMetadata { translation_metadata, sdv_publication_size }
    }

    pub(crate) fn translation_metadata(&self) -> &StructTypeMetadata {
        &self.translation_metadata
    }

    pub(crate) fn sdv_publication_size(&self) -> usize {
        self.sdv_publication_size
    }

    pub(crate) fn builder() -> SomeIpPublicationMetadataBuilder {
        SomeIpPublicationMetadataBuilder
    }
}
pub(crate) struct SomeIpPublicationMetadataBuilder;

impl SomeIpPublicationMetadataBuilder {
    pub(crate) fn build_publication_metadata(
        &self,
        publication_descriptor: &PublicationDescriptor,
        type_mapping: &SomeIpTypeMapping,
    ) -> anyhow::Result<SomeIpPublicationMetadata> {
        let publication_size = publication_descriptor.size_metadata.message_size as usize;
        match type_mapping.type_mapping() {
            None => Ok(SomeIpPublicationMetadata::new(
                StructTypeMetadata::new(LenFieldSize::ZeroBytes, vec![]),
                publication_size,
            )),
            Some(someip_struct_type_definition) => {
                let metadata = SomeIpMetadataBuilder::build_struct_type_metadata(
                    &publication_descriptor.message_descriptor,
                    someip_struct_type_definition,
                )?;
                Ok(SomeIpPublicationMetadata::new(metadata, publication_size))
            }
        }
    }

    pub(crate) fn build_publication_metadata_from_someip_definition(
        &self,
        publication_descriptor: &PublicationDescriptor,
        someip_struct_type_definition: &SomeIpStructTypeDefinition,
    ) -> anyhow::Result<SomeIpPublicationMetadata> {
        let publication_size = publication_descriptor.size_metadata.message_size as usize;
        let translation_metadata = SomeIpMetadataBuilder::build_struct_type_metadata(
            &publication_descriptor.message_descriptor,
            someip_struct_type_definition,
        )?;
        Ok(SomeIpPublicationMetadata::new(translation_metadata, publication_size))
    }
}

#[derive(Debug)]
pub(crate) struct SomeIpRpcMetadata {
    translation_metadata: StructTypeMetadata,
}

impl SomeIpRpcMetadata {
    pub(crate) fn get_translation_metadata(&self) -> &StructTypeMetadata {
        &self.translation_metadata
    }
}

pub(crate) struct SomeIpMetadataBuilder;

impl SomeIpMetadataBuilder {
    pub(crate) fn build_rpc_metadata(
        message_descriptor: Option<&MessageDescriptor>,
        someip_struct_type_definition: Option<&SomeIpStructTypeDefinition>,
    ) -> anyhow::Result<SomeIpRpcMetadata> {
        match (message_descriptor, someip_struct_type_definition) {
            (Some(descriptor), Some(mapping)) => Ok(SomeIpRpcMetadata {
                translation_metadata: Self::build_struct_type_metadata(descriptor, mapping)?,
            }),
            (None, None) => Ok(SomeIpRpcMetadata {
                translation_metadata: StructTypeMetadata {
                    someip_len_field_size: LenFieldSize::ZeroBytes,
                    fields: Vec::new(),
                },
            }),
            (None, Some(_)) => {
                Err(anyhow::anyhow!("Missing SOME/IP type definition for SOME/IP method."))
            }
            (Some(_), None) => {
                Err(anyhow::anyhow!("Missing SOME/IP method descriptor for SOME/IP method."))
            }
        }
    }

    fn build_struct_type_metadata(
        message_descriptor: &MessageDescriptor,
        someip_struct_type_definition: &SomeIpStructTypeDefinition,
    ) -> anyhow::Result<StructTypeMetadata> {
        let mut fields = Vec::new();
        if someip_struct_type_definition.length_field_size != LenFieldSize::ZeroBytes
            && someip_struct_type_definition.length_field_size != LenFieldSize::OneByte
            && someip_struct_type_definition.length_field_size != LenFieldSize::TwoBytes
            && someip_struct_type_definition.length_field_size != LenFieldSize::FourBytes
        {
            bail!(
                "Invalid length field size for struct size type: {:?}",
                someip_struct_type_definition.length_field_size
            )
        }

        for field in &someip_struct_type_definition.fields {
            let Some(field_descriptor) = message_descriptor.field_by_name(&field.field_name) else {
                bail!("Could not find protobuf field with name {}", field.field_name)
            };
            let field_number = field_descriptor.number() as u32;
            let is_repeated = field_descriptor.is_repeated();
            let protobuf_field_type = field_descriptor.proto().type_();
            match field.field_type {
                SomeIpMessageTypeDefinition::PRIMITIVE(primitive_type_def) => {
                    if is_repeated {
                        bail!("Cannot map a SOME/IP primitive type to a repeated field")
                    }
                    let primitive_type_metadata = Self::build_primitive_type_metadata(
                        primitive_type_def,
                        protobuf_field_type,
                    )?;
                    let field_metadata = FieldMetadata::new(
                        field_number,
                        FieldTypeMetadata::Primitive(primitive_type_metadata),
                    );
                    fields.push(field_metadata);
                }
                SomeIpMessageTypeDefinition::STRING(ref string_type_def) => {
                    if is_repeated {
                        bail!("Cannot map a SOME/IP string type to a repeated field")
                    }
                    let string_type_metadata =
                        Self::build_string_type_metadata(string_type_def, protobuf_field_type)?;
                    let field_metadata = FieldMetadata::new(
                        field_number,
                        FieldTypeMetadata::String(string_type_metadata),
                    );
                    fields.push(field_metadata);
                }
                SomeIpMessageTypeDefinition::ARRAY(ref array_type_def) => {
                    let array_type_metadata = Self::build_array_type_metadata(
                        array_type_def,
                        protobuf_field_type,
                        &field_descriptor,
                        is_repeated,
                    )?;
                    let field_metadata = FieldMetadata::new(
                        field_number,
                        FieldTypeMetadata::Array(array_type_metadata),
                    );
                    fields.push(field_metadata);
                }
                SomeIpMessageTypeDefinition::STRUCT(ref struct_type_def) => {
                    if is_repeated {
                        bail!("Cannot map a SOME/IP struct type to a repeated field")
                    }
                    let RuntimeType::Message(message_descriptor) =
                        field_descriptor.singular_runtime_type()
                    else {
                        bail!("SOME/IP structs can only map to protobuf messages")
                    };
                    let struct_field_type =
                        Self::build_struct_type_metadata(&message_descriptor, struct_type_def)?;
                    let field_metadata = FieldMetadata::new(
                        field_number,
                        FieldTypeMetadata::Struct(struct_field_type),
                    );
                    fields.push(field_metadata);
                }
            }
        }
        Ok(StructTypeMetadata::new(someip_struct_type_definition.length_field_size, fields))
    }

    fn build_primitive_type_metadata(
        primitive_type_def: SomeIpPrimitiveTypeDefinition,
        protobuf_field_type: Type,
    ) -> anyhow::Result<PrimitiveTypeMetadata> {
        if Self::is_not_primitive_protobuf_type(protobuf_field_type) {
            bail!("Expected a primitive protobuf type, found {:?}", protobuf_field_type);
        }
        PrimitiveTypeMetadata::new(primitive_type_def, protobuf_field_type)
    }

    fn build_string_type_metadata(
        string_type_definition: &SomeIpStringTypeDefinition,
        protobuf_field_type: Type,
    ) -> anyhow::Result<StringTypeMetadata> {
        if protobuf_field_type != Type::TYPE_STRING {
            bail!("Expected a string protobuf type, found {:?}", protobuf_field_type);
        }
        let someip_size_type = SomeIpSizeType::new(
            string_type_definition.fixed_length,
            string_type_definition.length_field_size,
        )?;
        Ok(StringTypeMetadata::new(someip_size_type))
    }

    fn build_array_type_metadata(
        array_type_definition: &SomeIpArrayTypeDefinition,
        protobuf_field_type: Type,
        field_descriptor: &FieldDescriptor,
        is_repeated: bool,
    ) -> anyhow::Result<ArrayTypeMetadata> {
        let size_type = SomeIpSizeType::new(
            array_type_definition.fixed_length,
            array_type_definition.length_field_size,
        )?;

        if !is_repeated {
            if protobuf_field_type != Type::TYPE_BYTES {
                bail!("SOME/IP array types can only be mapped to repeated fields or to bytes fields - found {:?} for field {:?}", protobuf_field_type, field_descriptor.name())
            }
            if SomeIpMessageTypeDefinition::PRIMITIVE(SomeIpPrimitiveTypeDefinition::U8)
                != *array_type_definition.array_type
            {
                bail!("Only an array of SOME/IP u8 can map to a protobuf bytes field, found {:?} for field {:?}", array_type_definition.array_type, field_descriptor.name())
            }
            Ok(ArrayTypeMetadata::new_bytes(size_type))
        } else {
            match *array_type_definition.array_type {
                SomeIpMessageTypeDefinition::PRIMITIVE(some_ip_primitive_type_definition) => {
                    ArrayTypeMetadata::new_primitive(
                        size_type,
                        some_ip_primitive_type_definition,
                        protobuf_field_type,
                    )
                }
                SomeIpMessageTypeDefinition::STRING(ref some_ip_string_type_definition) => {
                    let string_type_metadata = Self::build_string_type_metadata(
                        some_ip_string_type_definition,
                        protobuf_field_type,
                    )?;
                    Ok(ArrayTypeMetadata::new_string(size_type, string_type_metadata))
                }
                SomeIpMessageTypeDefinition::ARRAY(ref some_ip_array_type_definition) => {
                    // this can only be a repeated bytes field
                    if protobuf_field_type != Type::TYPE_BYTES {
                        bail!("SOME/IP 2d arrays can only be mapped to a repeated bytes field - found {:?} for field {:?}", protobuf_field_type, field_descriptor.name())
                    }
                    if SomeIpMessageTypeDefinition::PRIMITIVE(SomeIpPrimitiveTypeDefinition::U8)
                        != *some_ip_array_type_definition.array_type
                    {
                        bail!("Only a 2d array of SOME/IP u8 can map to a repeated bytes field - found {:?} for field {:?}", some_ip_array_type_definition.array_type, field_descriptor.name())
                    }
                    let nested_size_type = SomeIpSizeType::new(
                        some_ip_array_type_definition.fixed_length,
                        some_ip_array_type_definition.length_field_size,
                    )?;
                    Ok(ArrayTypeMetadata::new_repeated_bytes(size_type, nested_size_type))
                }
                SomeIpMessageTypeDefinition::STRUCT(ref some_ip_struct_type_definition) => {
                    let RuntimeFieldType::Repeated(RuntimeType::Message(message_descriptor)) =
                        field_descriptor.runtime_field_type()
                    else {
                        bail!("SOME/IP structs can only map to protobuf messages - found {} for field {:?}", field_descriptor, field_descriptor.name())
                    };
                    let struct_type = Self::build_struct_type_metadata(
                        &message_descriptor,
                        some_ip_struct_type_definition,
                    )?;
                    Ok(ArrayTypeMetadata::new_struct(size_type, struct_type))
                }
            }
        }
    }

    fn is_not_primitive_protobuf_type(protobuf_field_type: Type) -> bool {
        protobuf_field_type == Type::TYPE_STRING
            || protobuf_field_type == Type::TYPE_BYTES
            || protobuf_field_type == Type::TYPE_MESSAGE
            || protobuf_field_type == Type::TYPE_GROUP
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::direct::incoming::array_metadata::ArrayType;
    use protobuf::descriptor::{
        field_descriptor_proto::Label, DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    };
    use protobuf::reflect::FileDescriptor;
    use sdv_config_provider::vsidl_provider_api::SizeMetadata;
    use someip_config::serde_config::ffi::StringType;
    use someip_serde_reflect::SomeIpStructFieldTypeDefinition;

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
        let mut simple_msg = DescriptorProto::new();
        simple_msg.set_name("Simple".to_string());
        simple_msg.field.push(make_field("val", 1, Type::TYPE_UINT32, Label::LABEL_OPTIONAL, None));

        let mut complex_msg = DescriptorProto::new();
        complex_msg.set_name("Complex".to_string());
        complex_msg.field.push(make_field(
            "primitive_field",
            1,
            Type::TYPE_UINT32,
            Label::LABEL_OPTIONAL,
            None,
        ));
        complex_msg.field.push(make_field(
            "string_field",
            2,
            Type::TYPE_STRING,
            Label::LABEL_OPTIONAL,
            None,
        ));
        complex_msg.field.push(make_field(
            "bytes_field",
            3,
            Type::TYPE_BYTES,
            Label::LABEL_OPTIONAL,
            None,
        ));
        complex_msg.field.push(make_field(
            "repeated_primitive_field",
            4,
            Type::TYPE_UINT32,
            Label::LABEL_REPEATED,
            None,
        ));
        complex_msg.field.push(make_field(
            "repeated_string_field",
            5,
            Type::TYPE_STRING,
            Label::LABEL_REPEATED,
            None,
        ));
        complex_msg.field.push(make_field(
            "repeated_bytes_field",
            6,
            Type::TYPE_BYTES,
            Label::LABEL_REPEATED,
            None,
        ));
        complex_msg.field.push(make_field(
            "struct_field",
            7,
            Type::TYPE_MESSAGE,
            Label::LABEL_OPTIONAL,
            Some(".test.Simple"),
        ));
        complex_msg.field.push(make_field(
            "repeated_struct_field",
            8,
            Type::TYPE_MESSAGE,
            Label::LABEL_REPEATED,
            Some(".test.Simple"),
        ));

        let mut file_descriptor = FileDescriptorProto::new();
        file_descriptor.set_name("test.proto".to_string());
        file_descriptor.set_package("test".to_string());
        file_descriptor.message_type.push(simple_msg);
        file_descriptor.message_type.push(complex_msg);
        file_descriptor
    }

    fn get_message_descriptor(
        file_descriptor_proto: &FileDescriptorProto,
        message_name: &str,
    ) -> MessageDescriptor {
        let file_descriptor = FileDescriptor::new_dynamic(file_descriptor_proto.clone(), &[])
            .expect("Failed to create FileDescriptor");
        file_descriptor.message_by_package_relative_name(message_name).unwrap()
    }

    #[test]
    fn test_is_not_primitive_protobuf_type() {
        assert!(SomeIpMetadataBuilder::is_not_primitive_protobuf_type(Type::TYPE_STRING));
        assert!(SomeIpMetadataBuilder::is_not_primitive_protobuf_type(Type::TYPE_BYTES));
        assert!(SomeIpMetadataBuilder::is_not_primitive_protobuf_type(Type::TYPE_MESSAGE));
        assert!(SomeIpMetadataBuilder::is_not_primitive_protobuf_type(Type::TYPE_GROUP));
        assert!(!SomeIpMetadataBuilder::is_not_primitive_protobuf_type(Type::TYPE_UINT32));
        assert!(!SomeIpMetadataBuilder::is_not_primitive_protobuf_type(Type::TYPE_BOOL));
        assert!(!SomeIpMetadataBuilder::is_not_primitive_protobuf_type(Type::TYPE_FLOAT));
    }

    #[test]
    fn test_build_primitive_type_metadata_success() {
        let result = SomeIpMetadataBuilder::build_primitive_type_metadata(
            SomeIpPrimitiveTypeDefinition::U32,
            Type::TYPE_UINT32,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_primitive_type_metadata_not_primitive_proto() {
        let result = SomeIpMetadataBuilder::build_primitive_type_metadata(
            SomeIpPrimitiveTypeDefinition::U32,
            Type::TYPE_STRING,
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Expected a primitive protobuf type, found TYPE_STRING"
        );
    }

    #[test]
    fn test_build_primitive_type_metadata_invalid_mapping() {
        let result = SomeIpMetadataBuilder::build_primitive_type_metadata(
            SomeIpPrimitiveTypeDefinition::U32,
            Type::TYPE_BOOL,
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Type mismatch: SOME/IP type U32 cannot be mapped to proto type TYPE_BOOL"
        );
    }

    #[test]
    fn test_build_string_type_metadata_success_variable() {
        let someip_def = SomeIpStringTypeDefinition {
            fixed_length: None,
            bom: StringType::UTF_8,
            length_field_size: LenFieldSize::TwoBytes,
        };
        let result =
            SomeIpMetadataBuilder::build_string_type_metadata(&someip_def, Type::TYPE_STRING)
                .unwrap();
        assert_eq!(
            result,
            StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes))
        );
    }

    #[test]
    fn test_build_string_type_metadata_success_fixed() {
        let someip_def = SomeIpStringTypeDefinition {
            fixed_length: Some(10),
            bom: StringType::UTF_8,
            length_field_size: LenFieldSize::ZeroBytes, // ignored
        };
        let result =
            SomeIpMetadataBuilder::build_string_type_metadata(&someip_def, Type::TYPE_STRING)
                .unwrap();
        assert_eq!(result, StringTypeMetadata::new(SomeIpSizeType::FIXED(10)));
    }

    #[test]
    fn test_build_string_type_metadata_not_string_proto() {
        let someip_def = SomeIpStringTypeDefinition {
            fixed_length: None,
            bom: StringType::UTF_8,
            length_field_size: LenFieldSize::TwoBytes,
        };
        let result =
            SomeIpMetadataBuilder::build_string_type_metadata(&someip_def, Type::TYPE_UINT32);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Expected a string protobuf type, found TYPE_UINT32"
        );
    }

    #[test]
    fn test_build_string_type_metadata_invalid_size_type() {
        let someip_def = SomeIpStringTypeDefinition {
            fixed_length: None,
            bom: StringType::UTF_8,
            length_field_size: LenFieldSize::ZeroBytes, // Invalid for variable size
        };
        let result =
            SomeIpMetadataBuilder::build_string_type_metadata(&someip_def, Type::TYPE_STRING);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid length field size for variable size type"
        );
    }

    #[test]
    fn test_build_array_type_metadata_bytes_field() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");
        let field_descriptor = message_descriptor.field_by_name("bytes_field").unwrap();

        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
            fixed_length: None,
            length_field_size: LenFieldSize::FourBytes,
        };

        let result = SomeIpMetadataBuilder::build_array_type_metadata(
            &array_type_def,
            field_descriptor.proto().type_(),
            &field_descriptor,
            field_descriptor.is_repeated(),
        )
        .unwrap();

        assert_eq!(result.someip_size_type, SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes));
        assert_eq!(result.array_type, ArrayType::Bytes);
    }

    #[test]
    fn test_build_array_type_metadata_repeated_primitive() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");
        // repeated uint32
        let field_descriptor =
            message_descriptor.field_by_name("repeated_primitive_field").unwrap();

        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U32,
            )),
            fixed_length: Some(10), // 10 elements
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let result = SomeIpMetadataBuilder::build_array_type_metadata(
            &array_type_def,
            field_descriptor.proto().type_(),
            &field_descriptor,
            field_descriptor.is_repeated(),
        )
        .unwrap();

        assert_eq!(result.someip_size_type, SomeIpSizeType::FIXED(10));
    }

    #[test]
    fn test_build_array_type_metadata_repeated_string() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");
        let field_descriptor = message_descriptor.field_by_name("repeated_string_field").unwrap();

        // SOME/IP definition: array of string
        let string_type_def = SomeIpStringTypeDefinition {
            fixed_length: None,
            bom: StringType::UTF_8,
            length_field_size: LenFieldSize::TwoBytes, // Each string has a 2-byte length
        };

        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::STRING(string_type_def)),
            fixed_length: Some(5), // Array has fixed 5 elements
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let result = SomeIpMetadataBuilder::build_array_type_metadata(
            &array_type_def,
            field_descriptor.proto().type_(),
            &field_descriptor,
            field_descriptor.is_repeated(),
        )
        .unwrap();

        // Array size type
        assert_eq!(result.someip_size_type, SomeIpSizeType::FIXED(5));

        // String element type and size
        match result.array_type {
            ArrayType::String(string_metadata) => {
                assert_eq!(
                    string_metadata,
                    StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes))
                );
            }
            _ => panic!("Wrong array type, expected String"),
        }
    }

    #[test]
    fn test_build_array_type_metadata_repeated_bytes() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");
        let field_descriptor = message_descriptor.field_by_name("repeated_bytes_field").unwrap();

        // SOME/IP definition: array of (array of u8)
        let inner_array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
            fixed_length: Some(20), // Inner arrays are fixed size
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let outer_array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::ARRAY(inner_array_type_def)),
            fixed_length: None, // Outer array is variable size
            length_field_size: LenFieldSize::FourBytes,
        };

        let result = SomeIpMetadataBuilder::build_array_type_metadata(
            &outer_array_type_def,
            field_descriptor.proto().type_(),
            &field_descriptor,
            field_descriptor.is_repeated(),
        )
        .unwrap();

        // Outer array size type
        assert_eq!(result.someip_size_type, SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes));

        // Inner array type and size
        match result.array_type {
            ArrayType::RepeatedBytes(nested_size_type) => {
                assert_eq!(nested_size_type, SomeIpSizeType::FIXED(20));
            }
            _ => panic!("Wrong array type, expected RepeatedBytes"),
        }
    }

    #[test]
    fn test_build_array_type_metadata_invalid_size_type() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");
        let field_descriptor = message_descriptor.field_by_name("bytes_field").unwrap();

        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
            fixed_length: None,
            length_field_size: LenFieldSize::ZeroBytes, // Invalid for variable size
        };

        let result = SomeIpMetadataBuilder::build_array_type_metadata(
            &array_type_def,
            field_descriptor.proto().type_(),
            &field_descriptor,
            field_descriptor.is_repeated(),
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid length field size for variable size type"
        );
    }

    #[test]
    fn test_build_struct_type_metadata_success() {
        let someip_struct_def = SomeIpStructTypeDefinition {
            name: "Complex".to_string(),
            length_field_size: LenFieldSize::FourBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: "primitive_field".to_string(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
            }],
        };

        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");

        let result = SomeIpMetadataBuilder::build_struct_type_metadata(
            &message_descriptor,
            &someip_struct_def,
        )
        .unwrap();

        assert_eq!(result.someip_len_field_size, LenFieldSize::FourBytes);
        assert_eq!(result.fields.len(), 1);
        let field = &result.fields[0];
        assert_eq!(field.protobuf_field_number, 1);
        assert_eq!(
            field.field_type,
            FieldTypeMetadata::Primitive(
                PrimitiveTypeMetadata::new(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32)
                    .expect("PrimitiveTypeMetadata creation should succeed")
            )
        );
    }

    #[test]
    fn test_build_struct_type_metadata_multiple_fields() {
        let simple_struct_def = SomeIpStructTypeDefinition {
            name: "Simple".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: "val".to_string(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
            }],
        };

        let someip_struct_def = SomeIpStructTypeDefinition {
            name: "Complex".to_string(),
            length_field_size: LenFieldSize::FourBytes,
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "primitive_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U32,
                    ),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "repeated_primitive_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                        array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U32,
                        )),
                        fixed_length: None,
                        length_field_size: LenFieldSize::TwoBytes,
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "struct_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRUCT(simple_struct_def),
                },
            ],
        };

        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");

        let result = SomeIpMetadataBuilder::build_struct_type_metadata(
            &message_descriptor,
            &someip_struct_def,
        )
        .unwrap();

        assert_eq!(result.someip_len_field_size, LenFieldSize::FourBytes);
        assert_eq!(result.fields.len(), 3);

        // Check fields were built correctly
        assert_eq!(result.fields[0].protobuf_field_number, 1);
        assert_eq!(
            result.fields[0].field_type,
            FieldTypeMetadata::Primitive(
                PrimitiveTypeMetadata::new(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32)
                    .expect("PrimitiveTypeMetadata creation should succeed")
            )
        );

        assert_eq!(result.fields[1].protobuf_field_number, 4);
        assert_eq!(
            result.fields[1].field_type,
            FieldTypeMetadata::Array(
                ArrayTypeMetadata::new_primitive(
                    SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes),
                    SomeIpPrimitiveTypeDefinition::U32,
                    Type::TYPE_UINT32
                )
                .expect("ArrayMetadata creation should succeed")
            )
        );

        assert_eq!(result.fields[2].protobuf_field_number, 7);
        assert_eq!(
            result.fields[2].field_type,
            FieldTypeMetadata::Struct(StructTypeMetadata::new(
                LenFieldSize::ZeroBytes,
                vec![FieldMetadata::new(
                    1,
                    FieldTypeMetadata::Primitive(
                        PrimitiveTypeMetadata::new(
                            SomeIpPrimitiveTypeDefinition::U32,
                            Type::TYPE_UINT32
                        )
                        .expect("PrimitiveTypeMetadata creation should succeed")
                    )
                )]
            ))
        );
    }

    #[test]
    fn test_build_struct_type_metadata_field_not_found() {
        let someip_struct_def = SomeIpStructTypeDefinition {
            name: "Complex".to_string(),
            length_field_size: LenFieldSize::FourBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: "non_existent_field".to_string(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
            }],
        };

        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");

        let result = SomeIpMetadataBuilder::build_struct_type_metadata(
            &message_descriptor,
            &someip_struct_def,
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not find protobuf field with name non_existent_field"
        );
    }

    #[test]
    fn test_build_publication_metadata_no_mapping() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");
        let type_mapping = SomeIpTypeMapping::new(None);

        let size_metadata = SizeMetadata { message_size: 20, ..Default::default() };
        let publication_descriptor =
            PublicationDescriptor { message_descriptor: message_descriptor.clone(), size_metadata };

        let result = SomeIpPublicationMetadata::builder()
            .build_publication_metadata(&publication_descriptor, &type_mapping)
            .unwrap();

        assert_eq!(result.translation_metadata.someip_len_field_size, LenFieldSize::ZeroBytes);
        assert!(result.translation_metadata.fields.is_empty());
        assert_eq!(result.sdv_publication_size, 20);
    }

    #[test]
    fn test_build_publication_metadata_with_mapping() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Complex");

        let someip_struct_def = SomeIpStructTypeDefinition {
            name: "Complex".to_string(),
            length_field_size: LenFieldSize::FourBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: "primitive_field".to_string(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
            }],
        };
        let type_mapping = SomeIpTypeMapping::new(Some(someip_struct_def));

        let size_metadata = SizeMetadata { message_size: 100, ..Default::default() };
        let publication_descriptor =
            PublicationDescriptor { message_descriptor: message_descriptor.clone(), size_metadata };

        let result = SomeIpPublicationMetadata::builder()
            .build_publication_metadata(&publication_descriptor, &type_mapping)
            .unwrap();

        assert_eq!(result.translation_metadata.someip_len_field_size, LenFieldSize::FourBytes);
        assert_eq!(result.translation_metadata.fields.len(), 1);
        assert_eq!(result.sdv_publication_size, 100);
    }

    #[test]
    fn test_build_rpc_metadata_success() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Simple");

        let someip_struct_def = SomeIpStructTypeDefinition {
            name: "Simple".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: "val".to_string(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
            }],
        };

        let result = SomeIpMetadataBuilder::build_rpc_metadata(
            Some(&message_descriptor),
            Some(&someip_struct_def),
        )
        .unwrap();

        let translation_metadata = result.get_translation_metadata();
        assert_eq!(translation_metadata.someip_len_field_size, LenFieldSize::ZeroBytes);
        assert_eq!(translation_metadata.fields.len(), 1);
        assert_eq!(translation_metadata.fields[0].protobuf_field_number, 1);
    }

    #[test]
    fn test_build_rpc_metadata_no_params() {
        let result = SomeIpMetadataBuilder::build_rpc_metadata(None, None).unwrap();
        let translation_metadata = result.get_translation_metadata();
        assert_eq!(translation_metadata.someip_len_field_size, LenFieldSize::ZeroBytes);
        assert!(translation_metadata.fields.is_empty());
    }

    #[test]
    fn test_build_rpc_metadata_missing_descriptor() {
        let someip_struct_def = SomeIpStructTypeDefinition {
            name: "Simple".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![],
        };

        let result = SomeIpMetadataBuilder::build_rpc_metadata(None, Some(&someip_struct_def));

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Missing SOME/IP type definition for SOME/IP method."
        );
    }

    #[test]
    fn test_build_rpc_metadata_missing_mapping() {
        let file_proto = file_descriptor_for_test();
        let message_descriptor = get_message_descriptor(&file_proto, "Simple");

        let result = SomeIpMetadataBuilder::build_rpc_metadata(Some(&message_descriptor), None);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Missing SOME/IP method descriptor for SOME/IP method."
        );
    }
}
