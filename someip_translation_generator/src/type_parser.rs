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
