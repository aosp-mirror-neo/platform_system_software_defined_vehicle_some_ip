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

use someip_config::serde_config::ffi::{LenFieldSize, SomeIpConfig, StringType};
use someip_serde_reflect::{
    SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
    SomeIpStringTypeDefinition, SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
};
use vsidl_v1::someip_type::some_ip_type::LengthFieldSize;
use vsidl_v1::someip_type::some_ip_type::Someip_type;
use vsidl_v1::someip_type::some_ip_type::{some_ip_string::StringEncoding, SomeIpPrimitive};

pub(crate) struct TypeParser {}

impl TypeParser {
    pub(crate) fn parse(
        someip_config: &SomeIpConfig,
        someip_struct: Option<&vsidl_v1::someip_type::some_ip_type::SomeIpStruct>,
    ) -> anyhow::Result<Option<SomeIpStructTypeDefinition>> {
        let Some(someip_struct) = someip_struct else {
            return Ok(None);
        };
        Ok(Some(Self::get_someip_struct_type_definition(someip_config, someip_struct)?))
    }

    fn get_someip_type_definition(
        config: &SomeIpConfig,
        someip_type: &vsidl_v1::someip_type::SomeIpType,
    ) -> anyhow::Result<SomeIpMessageTypeDefinition> {
        match &someip_type.someip_type {
            Some(Someip_type::SomeipPrimitive(_)) => {
                Self::get_someip_primitive_type_definition(someip_type.someip_primitive())
                    .map(SomeIpMessageTypeDefinition::PRIMITIVE)
            }
            Some(Someip_type::SomeipString(someip_string)) => {
                Self::get_someip_string_type_definition(config, someip_string)
                    .map(SomeIpMessageTypeDefinition::STRING)
            }
            Some(Someip_type::SomeipArray(someip_array)) => {
                Self::get_someip_array_type_definition(config, someip_array)
                    .map(SomeIpMessageTypeDefinition::ARRAY)
            }
            Some(Someip_type::SomeipStruct(someip_struct)) => {
                Self::get_someip_struct_type_definition(config, someip_struct)
                    .map(SomeIpMessageTypeDefinition::STRUCT)
            }
            Some(_) | None => Err(anyhow::Error::msg(
                "SOME/IP message missing SOME/IP type information".to_string(),
            )),
        }
    }

    fn get_someip_primitive_type_definition(
        someip_primitive: vsidl_v1::someip_type::some_ip_type::SomeIpPrimitive,
    ) -> anyhow::Result<SomeIpPrimitiveTypeDefinition> {
        let primitive_type_definition = match someip_primitive {
            SomeIpPrimitive::UNKNOWN_TYPE => {
                return Err(anyhow::Error::msg("Unknown primitive type"));
            }
            SomeIpPrimitive::BOOL => SomeIpPrimitiveTypeDefinition::BOOLEAN,
            SomeIpPrimitive::INT8 => SomeIpPrimitiveTypeDefinition::I8,
            SomeIpPrimitive::INT16 => SomeIpPrimitiveTypeDefinition::I16,
            SomeIpPrimitive::INT32 => SomeIpPrimitiveTypeDefinition::I32,
            SomeIpPrimitive::INT64 => SomeIpPrimitiveTypeDefinition::I64,
            SomeIpPrimitive::UINT8 => SomeIpPrimitiveTypeDefinition::U8,
            SomeIpPrimitive::UINT16 => SomeIpPrimitiveTypeDefinition::U16,
            SomeIpPrimitive::UINT32 => SomeIpPrimitiveTypeDefinition::U32,
            SomeIpPrimitive::UINT64 => SomeIpPrimitiveTypeDefinition::U64,
            SomeIpPrimitive::FLOAT32 => SomeIpPrimitiveTypeDefinition::F32,
            SomeIpPrimitive::FLOAT64 => SomeIpPrimitiveTypeDefinition::F64,
        };
        Ok(primitive_type_definition)
    }

    fn get_someip_string_type_definition(
        config: &SomeIpConfig,
        someip_string: &vsidl_v1::someip_type::some_ip_type::SomeIpString,
    ) -> anyhow::Result<SomeIpStringTypeDefinition> {
        let enum_value = someip_string.string_encoding.enum_value();
        if let Ok(string_encoding) = enum_value {
            let bom = match string_encoding {
                StringEncoding::UNSPECIFIED_ENCODING => todo!(),
                StringEncoding::UTF_8 => StringType::UTF_8,
                StringEncoding::UTF_16_BE => StringType::UTF_16_BE,
                StringEncoding::UTF_16_LE => StringType::UTF_16_LE,
            };
            if someip_string.has_fixed_length() {
                Ok(SomeIpStringTypeDefinition {
                    length_field_size: LenFieldSize::ZeroBytes,
                    bom,
                    fixed_length: Option::Some(
                        someip_string.fixed_length().try_into().unwrap_or(usize::MAX),
                    ),
                })
            } else if someip_string.has_length_field_size() {
                let length_field_size =
                    Self::get_length_field(config, someip_string.length_field_size());
                Ok(SomeIpStringTypeDefinition {
                    length_field_size,
                    bom,
                    fixed_length: Option::None,
                })
            } else {
                Err(anyhow::Error::msg("String must be specified to either be fixed length or have a dynamic length field size".to_string()))
            }
        } else {
            Err(anyhow::Error::msg("Error while parsing SOME/IP String encoding value".to_string()))
        }
    }

    fn get_someip_array_type_definition(
        config: &SomeIpConfig,
        someip_array: &vsidl_v1::someip_type::some_ip_type::SomeIpArray,
    ) -> anyhow::Result<SomeIpArrayTypeDefinition> {
        let array_type = Self::get_someip_type_definition(config, &someip_array.array_type)?;
        if someip_array.has_fixed_length() {
            Ok(SomeIpArrayTypeDefinition {
                fixed_length: Option::Some(
                    someip_array.fixed_length().try_into().unwrap_or(usize::MAX),
                ),
                length_field_size: LenFieldSize::ZeroBytes,
                array_type: Box::new(array_type),
            })
        } else if someip_array.has_length_field_size() {
            let length_field_size =
                Self::get_length_field(config, someip_array.length_field_size());
            Ok(SomeIpArrayTypeDefinition {
                fixed_length: Option::None,
                length_field_size,
                array_type: Box::new(array_type),
            })
        } else {
            Err(anyhow::Error::msg("Array must be specified to either be fixed length or have a dynamic length field size".to_string()))
        }
    }

    fn get_someip_struct_type_definition(
        config: &SomeIpConfig,
        someip_struct: &vsidl_v1::someip_type::some_ip_type::SomeIpStruct,
    ) -> anyhow::Result<SomeIpStructTypeDefinition> {
        let length_field_size = Self::get_struct_length_field(config, someip_struct)?;
        let mut fields = Vec::new();
        for field in &someip_struct.field {
            let field_type = Self::get_someip_type_definition(config, &field.field_type)?;
            fields.push(SomeIpStructFieldTypeDefinition {
                data_tag: Option::None,
                field_name: field.field_name.clone(),
                field_type,
            });
        }
        Ok(SomeIpStructTypeDefinition { name: "".to_string(), length_field_size, fields })
    }

    fn get_struct_length_field(
        config: &SomeIpConfig,
        someip_struct: &vsidl_v1::someip_type::some_ip_type::SomeIpStruct,
    ) -> anyhow::Result<LenFieldSize> {
        let Some(len_field_size) = someip_struct.length_field_size else {
            return Ok(config.len_field_size);
        };
        let Ok(length_field_size) = len_field_size.enum_value() else {
            return Err(anyhow::Error::msg("Error while parsing struct length field".to_string()));
        };
        Ok(Self::get_length_field(config, length_field_size))
    }

    fn get_length_field(config: &SomeIpConfig, length_field_size: LengthFieldSize) -> LenFieldSize {
        match length_field_size {
            LengthFieldSize::UNSPECIFIED_SIZE => config.len_field_size,
            LengthFieldSize::ZERO => LenFieldSize::ZeroBytes,
            LengthFieldSize::ONE => LenFieldSize::OneByte,
            LengthFieldSize::TWO => LenFieldSize::TwoBytes,
            LengthFieldSize::FOUR => LenFieldSize::FourBytes,
        }
    }
}

#[cfg(test)]
mod tests {

    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};
    use someip_serde_reflect::{
        SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
        SomeIpStringTypeDefinition, SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
    };

    use vsidl_v1::someip_type::some_ip_type::some_ip_array::Array_length;
    use vsidl_v1::someip_type::some_ip_type::some_ip_string::{StringEncoding, String_length};
    use vsidl_v1::someip_type::some_ip_type::some_ip_struct::SomeIpStructField;
    use vsidl_v1::someip_type::some_ip_type::LengthFieldSize;
    use vsidl_v1::someip_type::some_ip_type::{
        SomeIpArray, SomeIpPrimitive, SomeIpString, SomeIpStruct,
    };
    use vsidl_v1::someip_type::SomeIpType;

    use crate::mapping_parser::type_parser::TypeParser;

    const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::ZeroBytes,
    };

    #[test]
    fn parse_empty_type_succeeds() {
        assert_eq!(None, TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, None).unwrap())
    }

    #[test]
    fn parse_someip_primitive_succeeds() {
        let field_name = String::from("a");
        let someip_primitive = SomeIpStruct {
            length_field_size: Some(LengthFieldSize::ZERO.into()),
            field: vec![SomeIpStructField {
                field_name: field_name.clone(),
                field_type: Some(SomeIpType {
                    someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipPrimitive(
                        SomeIpPrimitive::INT16.into(),
                    )
                    .into(),
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, Some(&someip_primitive)).unwrap();
        let expected = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: field_name.clone(),
                field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::I16,
                ),
            }],
        };
        assert_eq!(Some(expected), result)
    }

    #[test]
    fn parse_someip_string_succeeds() {
        let field_name = String::from("a");
        let someip_string = SomeIpStruct {
            length_field_size: Some(LengthFieldSize::ZERO.into()),
            field: vec![SomeIpStructField {
                field_name: field_name.clone(),
                field_type: Some(SomeIpType {
                    someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipString(
                        SomeIpString {
                            string_encoding: StringEncoding::UTF_8.into(),
                            string_length: Some(String_length::LengthFieldSize(
                                LengthFieldSize::ONE.into(),
                            )),
                            ..Default::default()
                        },
                    )
                    .into(),
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, Some(&someip_string)).unwrap();
        let expected = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: field_name.clone(),
                field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                    length_field_size: LenFieldSize::OneByte,
                    bom: StringType::UTF_8,
                    fixed_length: None,
                }),
            }],
        };
        assert_eq!(Some(expected), result)
    }

    #[test]
    fn parse_someip_array_succeeds() {
        let field_name = String::from("a");
        let someip_array = SomeIpStruct {
            length_field_size: Some(LengthFieldSize::ZERO.into()),
            field: vec![SomeIpStructField {
                field_name: field_name.clone(),
                field_type: Some(SomeIpType {
                    someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipArray(
                        SomeIpArray {
                            array_type: Some(SomeIpType {
                                someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipPrimitive(
                                    SomeIpPrimitive::INT16.into(),
                                )
                                .into(),
                                ..Default::default()
                            })
                            .into(),
                            array_length: Some(Array_length::LengthFieldSize(
                                LengthFieldSize::ONE.into(),
                            )),
                            ..Default::default()
                        },
                    )
                    .into(),
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, Some(&someip_array)).unwrap();
        let expected = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: field_name.clone(),
                field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                    fixed_length: None,
                    length_field_size: LenFieldSize::OneByte,
                    array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::I16,
                    )),
                }),
            }],
        };
        assert_eq!(Some(expected), result)
    }

    #[test]
    fn parse_someip_fixed_size_string_succeeds() {
        let field_name = String::from("fixed_string");
        let fixed_len = 128;
        let someip_fixed_string_struct = SomeIpStruct {
            length_field_size: Some(LengthFieldSize::ZERO.into()),
            field: vec![SomeIpStructField {
                field_name: field_name.clone(),
                field_type: Some(SomeIpType {
                    someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipString(
                        SomeIpString {
                            string_encoding: StringEncoding::UTF_8.into(),
                            string_length: Some(String_length::FixedLength(fixed_len)),
                            ..Default::default()
                        },
                    )
                    .into(),
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result =
            TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, Some(&someip_fixed_string_struct)).unwrap();
        let expected = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: field_name.clone(),
                field_type: SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                    length_field_size: LenFieldSize::ZeroBytes,
                    bom: StringType::UTF_8,
                    fixed_length: Some(fixed_len as usize),
                }),
            }],
        };
        assert_eq!(Some(expected), result)
    }

    #[test]
    fn parse_someip_fixed_size_array_succeeds() {
        let field_name = String::from("fixed_array");
        let fixed_len = 10;
        let someip_fixed_array_struct = SomeIpStruct {
            length_field_size: Some(LengthFieldSize::ZERO.into()),
            field: vec![SomeIpStructField {
                field_name: field_name.clone(),
                field_type: Some(SomeIpType {
                    someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipArray(
                        SomeIpArray {
                            array_type: Some(SomeIpType {
                                someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipPrimitive(
                                    SomeIpPrimitive::UINT32.into(),
                                )
                                .into(),
                                ..Default::default()
                            })
                            .into(),
                            array_length: Some(Array_length::FixedLength(fixed_len)),
                            ..Default::default()
                        },
                    )
                    .into(),
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result =
            TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, Some(&someip_fixed_array_struct)).unwrap();
        let expected = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: field_name.clone(),
                field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                    fixed_length: Some(fixed_len as usize),
                    length_field_size: LenFieldSize::ZeroBytes,
                    array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U32,
                    )),
                }),
            }],
        };
        assert_eq!(Some(expected), result)
    }

    #[test]
    fn parse_someip_nested_struct_succeeds() {
        let field_name = String::from("a");
        let nested_field_name = String::from("nested_field");
        let someip_nested_struct = SomeIpStruct {
            length_field_size: Some(LengthFieldSize::ZERO.into()),
            field: vec![SomeIpStructField {
                field_name: field_name.clone(),
                field_type: Some(SomeIpType {
                    someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipStruct(
                        SomeIpStruct {
                            length_field_size: Some(LengthFieldSize::ZERO.into()),
                            field: vec![
                                SomeIpStructField {
                                    field_name: nested_field_name.clone(),
                                    field_type: Some(SomeIpType {
                                        someip_type: vsidl_v1::someip_type::some_ip_type::Someip_type::SomeipPrimitive(
                                            SomeIpPrimitive::INT16.into(),
                                        )
                                        .into(),
                                        ..Default::default()
                                    }).into(),
                                    ..Default::default()
                                }
                            ],
                            ..Default::default()
                        }
                    )
                    .into(),
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result =
            TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, Some(&someip_nested_struct)).unwrap();
        let expected = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![SomeIpStructFieldTypeDefinition {
                data_tag: None,
                field_name: field_name.clone(),
                field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                    name: String::new(),
                    length_field_size: LenFieldSize::ZeroBytes,
                    fields: vec![SomeIpStructFieldTypeDefinition {
                        data_tag: None,
                        field_name: nested_field_name.clone(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::I16,
                        ),
                    }],
                }),
            }],
        };
        assert_eq!(Some(expected), result)
    }

    #[test]
    fn get_someip_primitive_type_definition_succeeds_and_fails() {
        // Test successful conversion
        let primitive_input = SomeIpPrimitive::INT32;
        let result = TypeParser::get_someip_primitive_type_definition(primitive_input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SomeIpPrimitiveTypeDefinition::I32);

        // Test error case for UNKNOWN_TYPE
        let unknown_primitive_input = SomeIpPrimitive::UNKNOWN_TYPE;
        let error_result =
            TypeParser::get_someip_primitive_type_definition(unknown_primitive_input);
        assert!(error_result.is_err());
        assert_eq!(error_result.unwrap_err().to_string(), "Unknown primitive type");
    }

    #[test]
    fn test_get_all_someip_primitive_type_definitions() {
        let test_cases = vec![
            (SomeIpPrimitive::BOOL, SomeIpPrimitiveTypeDefinition::BOOLEAN),
            (SomeIpPrimitive::INT8, SomeIpPrimitiveTypeDefinition::I8),
            (SomeIpPrimitive::INT16, SomeIpPrimitiveTypeDefinition::I16),
            (SomeIpPrimitive::INT32, SomeIpPrimitiveTypeDefinition::I32),
            (SomeIpPrimitive::INT64, SomeIpPrimitiveTypeDefinition::I64),
            (SomeIpPrimitive::UINT8, SomeIpPrimitiveTypeDefinition::U8),
            (SomeIpPrimitive::UINT16, SomeIpPrimitiveTypeDefinition::U16),
            (SomeIpPrimitive::UINT32, SomeIpPrimitiveTypeDefinition::U32),
            (SomeIpPrimitive::UINT64, SomeIpPrimitiveTypeDefinition::U64),
            (SomeIpPrimitive::FLOAT32, SomeIpPrimitiveTypeDefinition::F32),
            (SomeIpPrimitive::FLOAT64, SomeIpPrimitiveTypeDefinition::F64),
        ];

        for (input_primitive, expected_definition) in test_cases {
            let result = TypeParser::get_someip_primitive_type_definition(input_primitive);
            assert!(result.is_ok(), "Failed for input {:?}: {:?}", input_primitive, result.err());
            assert_eq!(
                result.unwrap(),
                expected_definition,
                "Mismatch for input {input_primitive:?}"
            );
        }
    }
}
