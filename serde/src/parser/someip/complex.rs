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

use serde::de::SeqAccess;
use serde::de::{DeserializeSeed, Error, Visitor};
use serde::Deserializer;
use someip_config::serde_config::ffi::{Endianness, LenFieldSize};

use crate::parser::common::LengthFieldDeserializeSeed;
use crate::parser::someip::array::ArrayValueDeserializeSeed;
use crate::parser::someip::primitive::PrimitiveValueSeed;
use crate::parser::someip::string::StringValueSeed;
use someip_serde_reflect::{
    SomeIpMessageData, SomeIpMessageTypeDefinition, SomeIpStructFieldType, SomeIpStructType,
    SomeIpStructTypeDefinition,
};

pub(crate) struct StructValueSeed<'lt> {
    someip_struct_definition: &'lt SomeIpStructTypeDefinition,
    endianness: Endianness,
}

impl<'lt> StructValueSeed<'lt> {
    pub(crate) fn new(
        someip_struct_definition: &'lt SomeIpStructTypeDefinition,
        endianness: Endianness,
    ) -> Self {
        StructValueSeed { someip_struct_definition, endianness }
    }
}

impl<'de> DeserializeSeed<'de> for StructValueSeed<'_> {
    type Value = SomeIpStructType;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StructVisitor<'lt> {
            someip_struct_definition: &'lt SomeIpStructTypeDefinition,
            endianness: Endianness,
        }

        impl<'lt> StructVisitor<'lt> {
            fn new(
                someip_struct_definition: &'lt SomeIpStructTypeDefinition,
                endianness: Endianness,
            ) -> Self {
                Self { someip_struct_definition, endianness }
            }
        }

        impl<'de> Visitor<'de> for StructVisitor<'_> {
            type Value = SomeIpStructType;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP struct type")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                match self.someip_struct_definition.length_field_size {
                    LenFieldSize::ZeroBytes => {
                        // OK to have no length field for structs
                        let mut fields = Vec::new();
                        for someip_field in &self.someip_struct_definition.fields {
                            match &someip_field.field_type {
                                SomeIpMessageTypeDefinition::PRIMITIVE(
                                    primitive_type_definition,
                                ) => {
                                    let Some(primitive_field) = seq.next_element_seed(
                                        PrimitiveValueSeed::new(primitive_type_definition),
                                    )?
                                    else {
                                        return Err(A::Error::custom(
                                            "Unable to deserialize struct primitive field",
                                        ));
                                    };
                                    fields.push(SomeIpStructFieldType {
                                        field_name: someip_field.field_name.clone(),
                                        field_data: SomeIpMessageData::PRIMITIVE(primitive_field),
                                    });
                                }
                                SomeIpMessageTypeDefinition::STRING(string_type_definition) => {
                                    let Some(string_field) = seq.next_element_seed(
                                        StringValueSeed::new(string_type_definition),
                                    )?
                                    else {
                                        return Err(A::Error::custom(
                                            "Unable to deserialize struct string field",
                                        ));
                                    };
                                    fields.push(SomeIpStructFieldType {
                                        field_name: someip_field.field_name.clone(),
                                        field_data: SomeIpMessageData::STRING(string_field),
                                    });
                                }
                                SomeIpMessageTypeDefinition::ARRAY(array_type_definition) => {
                                    let Some(array_field) =
                                        seq.next_element_seed(ArrayValueDeserializeSeed::new(
                                            array_type_definition,
                                            self.endianness,
                                        ))?
                                    else {
                                        return Err(A::Error::custom(
                                            "Unable to deserialize struct array field",
                                        ));
                                    };
                                    fields.push(SomeIpStructFieldType {
                                        field_name: someip_field.field_name.clone(),
                                        field_data: SomeIpMessageData::ARRAY(array_field),
                                    });
                                }
                                SomeIpMessageTypeDefinition::STRUCT(struct_type_definition) => {
                                    let Some(struct_field) =
                                        seq.next_element_seed(StructValueSeed::new(
                                            struct_type_definition,
                                            self.endianness,
                                        ))?
                                    else {
                                        return Err(A::Error::custom(
                                            "Unable to deserialize struct nested struct field",
                                        ));
                                    };
                                    fields.push(SomeIpStructFieldType {
                                        field_name: someip_field.field_name.clone(),
                                        field_data: SomeIpMessageData::STRUCT(struct_field),
                                    });
                                }
                            }
                        }
                        Ok(SomeIpStructType { fields })
                    }
                    _ => {
                        let Some(struct_size) =
                            seq.next_element_seed(LengthFieldDeserializeSeed::new(
                                &self.someip_struct_definition.length_field_size,
                            ))?
                        else {
                            return Err(A::Error::custom("Missing SOME/IP struct size"));
                        };
                        let Some(struct_type) =
                            seq.next_element_seed(StructValueWithLengthSeed::new(
                                self.someip_struct_definition,
                                self.endianness,
                                struct_size,
                            ))?
                        else {
                            return Err(A::Error::custom("Error deserializing struct value"));
                        };
                        Ok(struct_type)
                    }
                }
            }
        }
        deserializer
            .deserialize_seq(StructVisitor::new(self.someip_struct_definition, self.endianness))
    }
}

struct StructValueWithLengthSeed<'lt> {
    someip_struct_definition: &'lt SomeIpStructTypeDefinition,
    endianness: Endianness,
    struct_size: usize,
}

impl<'lt> StructValueWithLengthSeed<'lt> {
    fn new(
        someip_struct_definition: &'lt SomeIpStructTypeDefinition,
        endianness: Endianness,
        struct_size: usize,
    ) -> Self {
        Self { someip_struct_definition, endianness, struct_size }
    }
}

impl<'de> DeserializeSeed<'de> for StructValueWithLengthSeed<'_> {
    type Value = SomeIpStructType;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StructLengthVisitor<'lt> {
            someip_struct_definition: &'lt SomeIpStructTypeDefinition,
            endianness: Endianness,
        }

        impl<'lt> StructLengthVisitor<'lt> {
            fn new(
                someip_struct_definition: &'lt SomeIpStructTypeDefinition,
                endianness: Endianness,
            ) -> Self {
                Self { someip_struct_definition, endianness }
            }
        }

        impl<'de> Visitor<'de> for StructLengthVisitor<'_> {
            type Value = SomeIpStructType;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP struct type")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut fields = Vec::new();
                for someip_field in &self.someip_struct_definition.fields {
                    match &someip_field.field_type {
                        SomeIpMessageTypeDefinition::PRIMITIVE(primitive_type_definition) => {
                            let Some(primitive_field) = seq.next_element_seed(
                                PrimitiveValueSeed::new(primitive_type_definition),
                            )?
                            else {
                                return Err(A::Error::custom(
                                    "Unable to deserialize struct primitive field",
                                ));
                            };
                            fields.push(SomeIpStructFieldType {
                                field_name: someip_field.field_name.clone(),
                                field_data: SomeIpMessageData::PRIMITIVE(primitive_field),
                            });
                        }
                        SomeIpMessageTypeDefinition::STRING(string_type_definition) => {
                            let Some(string_field) = seq
                                .next_element_seed(StringValueSeed::new(string_type_definition))?
                            else {
                                return Err(A::Error::custom(
                                    "Unable to deserialize struct string field",
                                ));
                            };
                            fields.push(SomeIpStructFieldType {
                                field_name: someip_field.field_name.clone(),
                                field_data: SomeIpMessageData::STRING(string_field),
                            });
                        }
                        SomeIpMessageTypeDefinition::ARRAY(array_type_definition) => {
                            let Some(array_field) =
                                seq.next_element_seed(ArrayValueDeserializeSeed::new(
                                    array_type_definition,
                                    self.endianness,
                                ))?
                            else {
                                return Err(A::Error::custom(
                                    "Unable to deserialize struct array field",
                                ));
                            };
                            fields.push(SomeIpStructFieldType {
                                field_name: someip_field.field_name.clone(),
                                field_data: SomeIpMessageData::ARRAY(array_field),
                            });
                        }
                        SomeIpMessageTypeDefinition::STRUCT(struct_type_definition) => {
                            let Some(struct_field) = seq.next_element_seed(
                                StructValueSeed::new(struct_type_definition, self.endianness),
                            )?
                            else {
                                return Err(A::Error::custom(
                                    "Unable to deserialize struct nested struct field",
                                ));
                            };
                            fields.push(SomeIpStructFieldType {
                                field_name: someip_field.field_name.clone(),
                                field_data: SomeIpMessageData::STRUCT(struct_field),
                            });
                        }
                    }
                }
                Ok(SomeIpStructType { fields })
            }
        }

        deserializer.deserialize_tuple_struct(
            "",
            self.struct_size,
            StructLengthVisitor::new(self.someip_struct_definition, self.endianness),
        )
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{BigEndian, LittleEndian};
    use serde::de::DeserializeSeed;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, StringType};

    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::someip::complex::StructValueSeed;

    use someip_serde_reflect::{
        SomeIpArrayType, SomeIpArrayTypeDefinition, SomeIpMessageData, SomeIpMessageTypeDefinition,
        SomeIpPrimitiveType, SomeIpPrimitiveTypeDefinition, SomeIpStringTypeDefinition,
        SomeIpStructFieldType, SomeIpStructFieldTypeDefinition, SomeIpStructType,
        SomeIpStructTypeDefinition,
    };

    const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];

    #[test]
    fn test_struct_deserialize_succeeds() {
        let mut payload = Vec::new();

        // struct with len field size of 4 bytes with 1 u64 field
        payload.extend_from_slice(&8u32.to_be_bytes());
        payload.extend_from_slice(&456u64.to_be_bytes());

        // array
        let array_data: Vec<i64> = vec![6373, 1290, 3529];
        payload.push(24);
        for element in &array_data {
            payload.extend_from_slice(&element.to_be_bytes());
        }

        // string
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        payload.push(3 + 5 + 1);
        payload.extend_from_slice(BOM_UTF8);
        payload.extend_from_slice(text_bytes);
        payload.push(0x00);

        // primitive
        payload.extend_from_slice(&2023u32.to_be_bytes());

        let struct_type_definition = SomeIpStructTypeDefinition {
            name: "Test".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "nested_struct".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::FourBytes,
                        fields: vec![SomeIpStructFieldTypeDefinition {
                            data_tag: Option::None,
                            field_name: "nested_field".to_string(),
                            field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                                SomeIpPrimitiveTypeDefinition::U64,
                            ),
                        }],
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "arr".to_string(),
                    field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                        fixed_length: None,
                        length_field_size: LenFieldSize::OneByte,
                        array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::I64,
                        )),
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "str".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                        length_field_size: LenFieldSize::OneByte,
                        bom: StringType::UTF_8,
                        fixed_length: None,
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "u32".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U32,
                    ),
                },
            ],
        };

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let struct_value_seed =
            StructValueSeed::new(&struct_type_definition, Endianness::BigEndian);

        let result =
            struct_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpStructType {
                fields: vec![
                    SomeIpStructFieldType {
                        field_name: "nested_struct".to_string(),
                        field_data: SomeIpMessageData::STRUCT(SomeIpStructType {
                            fields: vec![SomeIpStructFieldType {
                                field_name: "nested_field".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U64(
                                    456
                                ))
                            }]
                        })
                    },
                    SomeIpStructFieldType {
                        field_name: "arr".to_string(),
                        field_data: SomeIpMessageData::ARRAY(SomeIpArrayType {
                            array_data: vec![
                                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(
                                    array_data[0]
                                )),
                                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(
                                    array_data[1]
                                )),
                                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(
                                    array_data[2]
                                )),
                            ]
                        })
                    },
                    SomeIpStructFieldType {
                        field_name: "str".to_string(),
                        field_data: SomeIpMessageData::STRING(text)
                    },
                    SomeIpStructFieldType {
                        field_name: "u32".to_string(),
                        field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U32(2023))
                    }
                ]
            }
        );
    }

    #[test]
    fn test_struct_deserialize_little_endian_succeeds() {
        let mut payload = Vec::new();

        // Nested struct with len field size of 4 bytes with 1 u64 field
        payload.extend_from_slice(&8u32.to_le_bytes()); // Length of nested_field (u64 = 8 bytes)
        payload.extend_from_slice(&456u64.to_le_bytes()); // Data of nested_field

        // array
        let array_data: Vec<i64> = vec![6373, 1290, 3529];
        payload.push(24); // Length of array_data (3 elements * 8 bytes/i64 = 24 bytes)
        for element in &array_data {
            payload.extend_from_slice(&element.to_le_bytes()); // Data of array elements
        }

        // string
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        payload.push((BOM_UTF8.len() + text_bytes.len() + 1) as u8); // Length of string data (BOM + "hello" + NUL)
        payload.extend_from_slice(BOM_UTF8);
        payload.extend_from_slice(text_bytes);
        payload.push(0x00);

        // primitive
        payload.extend_from_slice(&2023u32.to_le_bytes()); // Data of u32 primitive

        let struct_type_definition = SomeIpStructTypeDefinition {
            name: "Test".to_string(),
            length_field_size: LenFieldSize::ZeroBytes, // Outer struct has no length field
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "nested_struct".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::FourBytes, // Nested struct has 4-byte length
                        fields: vec![SomeIpStructFieldTypeDefinition {
                            data_tag: Option::None,
                            field_name: "nested_field".to_string(),
                            field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                                SomeIpPrimitiveTypeDefinition::U64,
                            ),
                        }],
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "arr".to_string(),
                    field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                        fixed_length: None,
                        length_field_size: LenFieldSize::OneByte,
                        array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::I64,
                        )),
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "str".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                        length_field_size: LenFieldSize::OneByte,
                        bom: StringType::UTF_8,
                        fixed_length: None,
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "u32".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U32,
                    ),
                },
            ],
        };

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let struct_value_seed =
            StructValueSeed::new(&struct_type_definition, Endianness::LittleEndian);

        let result =
            struct_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpStructType {
                fields: vec![
                    SomeIpStructFieldType {
                        field_name: "nested_struct".to_string(),
                        field_data: SomeIpMessageData::STRUCT(SomeIpStructType {
                            fields: vec![SomeIpStructFieldType {
                                field_name: "nested_field".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U64(
                                    456
                                ))
                            }]
                        })
                    },
                    SomeIpStructFieldType {
                        field_name: "arr".to_string(),
                        field_data: SomeIpMessageData::ARRAY(SomeIpArrayType {
                            array_data: vec![
                                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(
                                    array_data[0]
                                )),
                                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(
                                    array_data[1]
                                )),
                                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(
                                    array_data[2]
                                )),
                            ]
                        })
                    },
                    SomeIpStructFieldType {
                        field_name: "str".to_string(),
                        field_data: SomeIpMessageData::STRING(text)
                    },
                    SomeIpStructFieldType {
                        field_name: "u32".to_string(),
                        field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U32(2023))
                    }
                ]
            }
        );
    }

    #[test]
    fn test_struct_with_two_byte_length_field_deserialize_succeeds() {
        // Define the structure of the SOME/IP message
        let nested_struct_val = 456u64;
        let array_data_vals: Vec<i64> = vec![6373, 1290, 3529];
        let text_val = String::from("hello");
        let u32_primitive_val = 2023u32;

        let struct_type_definition = SomeIpStructTypeDefinition {
            name: "OuterStructWithTwoByteLength".to_string(),
            length_field_size: LenFieldSize::TwoBytes, // Outer struct has a 2-byte length field
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "nested_struct".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                        name: "Nested".to_string(),
                        length_field_size: LenFieldSize::FourBytes, // Nested struct has 4-byte length
                        fields: vec![SomeIpStructFieldTypeDefinition {
                            data_tag: Option::None,
                            field_name: "nested_field".to_string(),
                            field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                                SomeIpPrimitiveTypeDefinition::U64,
                            ),
                        }],
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "arr".to_string(),
                    field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                        fixed_length: None,
                        length_field_size: LenFieldSize::OneByte,
                        array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::I64,
                        )),
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "str".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                        length_field_size: LenFieldSize::OneByte,
                        bom: StringType::UTF_8,
                        fixed_length: None,
                    }),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: Option::None,
                    field_name: "u32_primitive".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U32,
                    ),
                },
            ],
        };

        let expected_struct_data = SomeIpStructType {
            fields: vec![
                SomeIpStructFieldType {
                    field_name: "nested_struct".to_string(),
                    field_data: SomeIpMessageData::STRUCT(SomeIpStructType {
                        fields: vec![SomeIpStructFieldType {
                            field_name: "nested_field".to_string(),
                            field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U64(
                                nested_struct_val,
                            )),
                        }],
                    }),
                },
                SomeIpStructFieldType {
                    field_name: "arr".to_string(),
                    field_data: SomeIpMessageData::ARRAY(SomeIpArrayType {
                        array_data: array_data_vals
                            .iter()
                            .map(|&val| SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(val)))
                            .collect(),
                    }),
                },
                SomeIpStructFieldType {
                    field_name: "str".to_string(),
                    field_data: SomeIpMessageData::STRING(text_val.clone()),
                },
                SomeIpStructFieldType {
                    field_name: "u32_primitive".to_string(),
                    field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U32(
                        u32_primitive_val,
                    )),
                },
            ],
        };

        // --- Big Endian Test ---
        let mut fields_payload_be = Vec::new();
        fields_payload_be.extend_from_slice(&8u32.to_be_bytes()); // nested_struct length
        fields_payload_be.extend_from_slice(&nested_struct_val.to_be_bytes()); // nested_struct data
        fields_payload_be.push((array_data_vals.len() * 8) as u8); // arr length
        for val in &array_data_vals {
            fields_payload_be.extend_from_slice(&val.to_be_bytes());
        }
        fields_payload_be.push((BOM_UTF8.len() + text_val.len() + 1) as u8); // str length
        fields_payload_be.extend_from_slice(BOM_UTF8);
        fields_payload_be.extend_from_slice(text_val.as_bytes());
        fields_payload_be.push(0x00);
        fields_payload_be.extend_from_slice(&u32_primitive_val.to_be_bytes()); // u32_primitive data

        let mut payload_be = (fields_payload_be.len() as u16).to_be_bytes().to_vec(); // Outer struct length (2 bytes BE)
        payload_be.append(&mut fields_payload_be);

        let mut deserializer_be = SomeIpSerdeDeserializer::<BigEndian>::new(&payload_be);
        let struct_seed_be = StructValueSeed::new(&struct_type_definition, Endianness::BigEndian);
        let result_be =
            struct_seed_be.deserialize(&mut deserializer_be).expect("BE Deserialization failed");
        assert_eq!(result_be, expected_struct_data);

        // --- Little Endian Test ---
        let mut fields_payload_le = Vec::new();
        fields_payload_le.extend_from_slice(&8u32.to_le_bytes()); // nested_struct length
        fields_payload_le.extend_from_slice(&nested_struct_val.to_le_bytes()); // nested_struct data
        fields_payload_le.push((array_data_vals.len() * 8) as u8); // arr length
        for val in &array_data_vals {
            fields_payload_le.extend_from_slice(&val.to_le_bytes());
        }
        fields_payload_le.push((BOM_UTF8.len() + text_val.len() + 1) as u8); // str length
        fields_payload_le.extend_from_slice(BOM_UTF8);
        fields_payload_le.extend_from_slice(text_val.as_bytes());
        fields_payload_le.push(0x00);
        fields_payload_le.extend_from_slice(&u32_primitive_val.to_le_bytes()); // u32_primitive data

        let mut payload_le = (fields_payload_le.len() as u16).to_le_bytes().to_vec(); // Outer struct length (2 bytes LE)
        payload_le.append(&mut fields_payload_le);

        let mut deserializer_le = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload_le);
        let struct_seed_le =
            StructValueSeed::new(&struct_type_definition, Endianness::LittleEndian);
        let result_le =
            struct_seed_le.deserialize(&mut deserializer_le).expect("LE Deserialization failed");
        assert_eq!(result_le, expected_struct_data);
    }
}
