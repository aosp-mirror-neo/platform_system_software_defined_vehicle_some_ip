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

use protobuf::reflect::{MessageDescriptor, ReflectValueRef};
use protobuf::MessageDyn;

use serde::ser::{Error, SerializeSeq};
use serde::Serialize;
use someip_config::serde_config::ffi::LenFieldSize;

use crate::serializer::array::ArrayValueSeed;
use crate::serializer::get_byte_size;
use crate::serializer::primitive::PrimitiveValueSeed;
use crate::serializer::string::StringValueSeed;
use someip_serde_reflect::{SomeIpMessageTypeDefinition, SomeIpStructTypeDefinition};

pub(crate) struct StructValueSeed<'lt> {
    someip_struct_definition: &'lt SomeIpStructTypeDefinition,
    proto_message_descriptor: &'lt MessageDescriptor,
    message: &'lt dyn MessageDyn,
}

impl<'lt> StructValueSeed<'lt> {
    pub(crate) fn new(
        someip_struct_definition: &'lt SomeIpStructTypeDefinition,
        proto_message_descriptor: &'lt MessageDescriptor,
        message: &'lt dyn MessageDyn,
    ) -> Self {
        StructValueSeed { someip_struct_definition, proto_message_descriptor, message }
    }
}

impl Serialize for StructValueSeed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let optional_size =
            if self.someip_struct_definition.length_field_size == LenFieldSize::ZeroBytes {
                None
            } else {
                Some(
                    get_byte_size(&self.someip_struct_definition.length_field_size)
                        .map_err(|e| S::Error::custom(e.to_string()))?,
                )
            };

        let mut seq = serializer.serialize_seq(optional_size)?;

        for someip_field in &self.someip_struct_definition.fields {
            let Some(field) = self.proto_message_descriptor.field_by_name(&someip_field.field_name)
            else {
                return Err(S::Error::custom(format!(
                    "Missing protobuf definition for field name {}",
                    someip_field.field_name
                )));
            };
            match &someip_field.field_type {
                SomeIpMessageTypeDefinition::PRIMITIVE(primitive_type_definition) => {
                    if field.is_singular() {
                        seq.serialize_element(&PrimitiveValueSeed::new(
                            primitive_type_definition,
                            field.get_singular_field_or_default(self.message),
                        ))?;
                    } else {
                        return Err(S::Error::custom(format!(
                            "Expected a singular proto field for field {}",
                            someip_field.field_name
                        )));
                    }
                }
                SomeIpMessageTypeDefinition::STRING(string_type_definition) => {
                    if field.is_singular() {
                        seq.serialize_element(&StringValueSeed::new(
                            string_type_definition,
                            field.get_singular_field_or_default(self.message),
                        ))?;
                    } else {
                        return Err(S::Error::custom(format!(
                            "Expected a singular proto field for field {}",
                            someip_field.field_name
                        )));
                    }
                }
                SomeIpMessageTypeDefinition::ARRAY(array_type_definition) => {
                    seq.serialize_element(&ArrayValueSeed::new(
                        array_type_definition,
                        &field,
                        self.message,
                    ))?;
                }
                SomeIpMessageTypeDefinition::STRUCT(struct_type_definition) => {
                    if field.is_singular() {
                        let proto_field_value = field.get_singular_field_or_default(self.message);

                        if let ReflectValueRef::Message(msg) = proto_field_value {
                            let nested_message_descriptor = msg.descriptor_dyn();

                            seq.serialize_element(&StructValueSeed::new(
                                struct_type_definition,
                                &nested_message_descriptor,
                                &*msg,
                            ))?;
                        } else {
                            return Err(S::Error::custom(format!(
                                "Expected a message proto field for field {}",
                                someip_field.field_name
                            )));
                        }
                    } else {
                        return Err(S::Error::custom(format!(
                            "Expected a singular proto field for field {}",
                            someip_field.field_name
                        )));
                    }
                }
            }
        }

        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serializer::string::BOM_UTF8;
    use crate::serializer::SomeIpSerdeSerializer;
    use byteorder::BigEndian;
    use protobuf::MessageDyn;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};
    use someip_serde_reflect::{
        SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
        SomeIpStringTypeDefinition, SomeIpStructFieldTypeDefinition,
    };
    use someip_serde_test_protos::messages::{SimpleTestMessage, TestStruct};

    const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8, // Default, can be overridden by StringTypeDefinition
        len_field_size: LenFieldSize::OneByte, // Default for serializer in these tests
    };

    // Helper to create the SOME/IP definition for SimpleTestMessage
    fn get_simple_test_message_someip_def() -> SomeIpStructTypeDefinition {
        SomeIpStructTypeDefinition {
            name: "SimpleTestMessage".to_string(),
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    field_name: "a".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U64,
                    ),
                    data_tag: None,
                },
                SomeIpStructFieldTypeDefinition {
                    field_name: "b".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::F64,
                    ),
                    data_tag: None,
                },
                SomeIpStructFieldTypeDefinition {
                    field_name: "c".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U8,
                    ),
                    data_tag: None,
                },
            ],
            length_field_size: LenFieldSize::ZeroBytes, // Fixed length for this nested struct
        }
    }

    #[test]
    fn test_serialize_test_struct_dynamic_length() {
        let simple_test_msg_someip_def = get_simple_test_message_someip_def();

        let test_struct_someip_def = SomeIpStructTypeDefinition {
            name: "TestStruct".to_string(),
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    field_name: "string_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                        length_field_size: LenFieldSize::OneByte, // String's own length
                        bom: StringType::UTF_8,
                        fixed_length: None,
                    }),
                    data_tag: None,
                },
                SomeIpStructFieldTypeDefinition {
                    field_name: "array_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                        array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U32,
                        )),
                        length_field_size: LenFieldSize::OneByte, // Array's own content length
                        fixed_length: None,
                    }),
                    data_tag: None,
                },
                SomeIpStructFieldTypeDefinition {
                    field_name: "struct_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRUCT(
                        simple_test_msg_someip_def.clone(),
                    ),
                    data_tag: None,
                },
            ],
            length_field_size: LenFieldSize::TwoBytes, // TestStruct's own length
        };

        let mut test_struct_msg = TestStruct::new();
        test_struct_msg.string_field = "hello".to_string();
        test_struct_msg.array_field = vec![100, 200];
        let mut simple_msg = SimpleTestMessage::new();
        simple_msg.a = 1;
        simple_msg.b = 2.0;
        simple_msg.c = 3;
        test_struct_msg.struct_field = Some(simple_msg.clone()).into();

        let msg_dyn: &dyn MessageDyn = &test_struct_msg;
        let descriptor = test_struct_msg.descriptor_dyn();

        let seed = StructValueSeed::new(&test_struct_someip_def, &descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();

        let string_payload_data_len: u8 = (BOM_UTF8.len() + "hello".len() + 1) as u8;
        let mut string_bytes_part = Vec::new();
        string_bytes_part.push(string_payload_data_len);
        string_bytes_part.extend_from_slice(BOM_UTF8);
        string_bytes_part.extend_from_slice("hello".as_bytes());
        string_bytes_part.push(0x00);

        let array_payload_data_len: u8 =
            (test_struct_msg.array_field.len() * std::mem::size_of::<u32>()) as u8;
        let mut array_bytes_part = Vec::new();
        array_bytes_part.push(array_payload_data_len);
        for val in &test_struct_msg.array_field {
            array_bytes_part.extend_from_slice(&val.to_be_bytes());
        }

        let mut struct_bytes_part = Vec::new();
        struct_bytes_part.extend_from_slice(&simple_msg.a.to_be_bytes());
        struct_bytes_part.extend_from_slice(&simple_msg.b.to_be_bytes());
        struct_bytes_part.push(simple_msg.c as u8);

        let total_payload_len: u16 =
            (string_bytes_part.len() + array_bytes_part.len() + struct_bytes_part.len()) as u16;
        expected_bytes.extend_from_slice(&total_payload_len.to_be_bytes());
        expected_bytes.extend_from_slice(&string_bytes_part);
        expected_bytes.extend_from_slice(&array_bytes_part);
        expected_bytes.extend_from_slice(&struct_bytes_part);

        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }

    #[test]
    fn test_serialize_test_struct_fixed_length() {
        let simple_test_msg_someip_def = get_simple_test_message_someip_def();

        let string_fixed_len = 10;
        let array_fixed_num_elements = 2;

        let test_struct_someip_def = SomeIpStructTypeDefinition {
            name: "TestStruct".to_string(),
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    field_name: "string_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                        length_field_size: LenFieldSize::ZeroBytes,
                        bom: StringType::UTF_8,
                        fixed_length: Some(string_fixed_len),
                    }),
                    data_tag: None,
                },
                SomeIpStructFieldTypeDefinition {
                    field_name: "array_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                        array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U32,
                        )),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fixed_length: Some(array_fixed_num_elements),
                    }),
                    data_tag: None,
                },
                SomeIpStructFieldTypeDefinition {
                    field_name: "struct_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRUCT(
                        simple_test_msg_someip_def.clone(),
                    ),
                    data_tag: None,
                },
            ],
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let mut test_struct_msg = TestStruct::new();
        test_struct_msg.string_field = "hi".to_string();
        test_struct_msg.array_field = vec![10, 20];
        assert_eq!(test_struct_msg.array_field.len(), array_fixed_num_elements);
        let mut simple_msg = SimpleTestMessage::new();
        simple_msg.a = 1;
        simple_msg.b = 2.0;
        simple_msg.c = 3;
        test_struct_msg.struct_field = Some(simple_msg.clone()).into();

        let msg_dyn: &dyn MessageDyn = &test_struct_msg;
        let descriptor = test_struct_msg.descriptor_dyn();

        let seed = StructValueSeed::new(&test_struct_someip_def, &descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();

        let mut string_bytes_part = Vec::new();
        string_bytes_part.extend_from_slice(BOM_UTF8);
        string_bytes_part.extend_from_slice("hi".as_bytes());
        string_bytes_part.push(0x00);
        string_bytes_part.resize(string_fixed_len, 0x00);
        expected_bytes.extend_from_slice(&string_bytes_part);

        for val in &test_struct_msg.array_field {
            expected_bytes.extend_from_slice(&val.to_be_bytes());
        }

        expected_bytes.extend_from_slice(&simple_msg.a.to_be_bytes());
        expected_bytes.extend_from_slice(&simple_msg.b.to_be_bytes());
        expected_bytes.push(simple_msg.c as u8);

        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }

    #[test]
    fn test_serialize_error_missing_proto_field() {
        let test_struct_someip_def = SomeIpStructTypeDefinition {
            name: "TestStruct".to_string(),
            fields: vec![SomeIpStructFieldTypeDefinition {
                field_name: "non_existent_field".to_string(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32,
                ),
                data_tag: None,
            }],
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let test_struct_msg = TestStruct::new();
        let msg_dyn: &dyn MessageDyn = &test_struct_msg;
        let descriptor = test_struct_msg.descriptor_dyn();

        let seed = StructValueSeed::new(&test_struct_someip_def, &descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        let result = seed.serialize(&mut serializer);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Missing protobuf definition for field name non_existent_field"
        );
    }

    #[test]
    fn test_serialize_error_primitive_expects_singular_gets_repeated() {
        let test_struct_someip_def = SomeIpStructTypeDefinition {
            name: "TestStruct".to_string(),
            fields: vec![SomeIpStructFieldTypeDefinition {
                field_name: "array_field".to_string(), // 'repeated uint32' in proto
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U32, // SOME/IP expects singular
                ),
                data_tag: None,
            }],
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let mut test_struct_msg = TestStruct::new();
        test_struct_msg.array_field = vec![1, 2];
        let msg_dyn: &dyn MessageDyn = &test_struct_msg;
        let descriptor = test_struct_msg.descriptor_dyn();

        let seed = StructValueSeed::new(&test_struct_someip_def, &descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        let result = seed.serialize(&mut serializer);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Expected a singular proto field for field array_field"
        );
    }

    #[test]
    fn test_serialize_error_string_expects_string_gets_uint64() {
        let test_struct_someip_def = SomeIpStructTypeDefinition {
            name: "SimpleTestMessage".to_string(),
            fields: vec![SomeIpStructFieldTypeDefinition {
                field_name: "a".to_string(), // 'uint64' in proto
                field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                    length_field_size: LenFieldSize::OneByte,
                    bom: StringType::UTF_8,
                    fixed_length: None,
                }),
                data_tag: None,
            }],
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let mut simple_msg = SimpleTestMessage::new();
        simple_msg.a = 12345;
        let msg_dyn: &dyn MessageDyn = &simple_msg;
        let descriptor = simple_msg.descriptor_dyn();

        let seed = StructValueSeed::new(&test_struct_someip_def, &descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        let result = seed.serialize(&mut serializer);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Expected a string proto value"
        );
    }

    #[test]
    fn test_serialize_error_struct_expects_message_gets_uint64() {
        let simple_test_msg_someip_def = get_simple_test_message_someip_def();
        let test_struct_someip_def = SomeIpStructTypeDefinition {
            name: "SimpleTestMessage".to_string(),
            fields: vec![SomeIpStructFieldTypeDefinition {
                field_name: "a".to_string(), // 'uint64' in proto
                field_type: SomeIpMessageTypeDefinition::STRUCT(simple_test_msg_someip_def.clone()),
                data_tag: None,
            }],
            length_field_size: LenFieldSize::ZeroBytes,
        };

        let mut simple_msg_outer = SimpleTestMessage::new();
        simple_msg_outer.a = 12345;
        let msg_dyn: &dyn MessageDyn = &simple_msg_outer;
        let descriptor = simple_msg_outer.descriptor_dyn();

        let seed = StructValueSeed::new(&test_struct_someip_def, &descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        let result = seed.serialize(&mut serializer);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Expected a message proto field for field a"
        );
    }
}
