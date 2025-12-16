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

use protobuf::reflect::ReflectValueRef;
use serde::ser::Error;
use serde::Serialize;

use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

pub(crate) struct PrimitiveValueSeed<'lt> {
    primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
    proto_field_value: ReflectValueRef<'lt>,
}

impl<'lt> PrimitiveValueSeed<'lt> {
    pub(crate) fn new(
        primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
        proto_field_value: ReflectValueRef<'lt>,
    ) -> Self {
        Self { primitive_type_definition, proto_field_value }
    }
}

impl Serialize for PrimitiveValueSeed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match (self.primitive_type_definition, &self.proto_field_value) {
            (SomeIpPrimitiveTypeDefinition::BOOLEAN, ReflectValueRef::Bool(v)) => {
                serializer.serialize_bool(*v)
            }
            (SomeIpPrimitiveTypeDefinition::U8, ReflectValueRef::U32(v)) => {
                match u8::try_from(*v) {
                    Ok(value) => serializer.serialize_u8(value),
                    Err(_) => Err(S::Error::custom("Error translating u32 value to u8")),
                }
            }
            (SomeIpPrimitiveTypeDefinition::U16, ReflectValueRef::U32(v)) => {
                match u16::try_from(*v) {
                    Ok(value) => serializer.serialize_u16(value),
                    Err(_) => Err(S::Error::custom("Error translating u32 value to u16")),
                }
            }
            (SomeIpPrimitiveTypeDefinition::U32, ReflectValueRef::U32(v)) => {
                serializer.serialize_u32(*v)
            }
            (SomeIpPrimitiveTypeDefinition::U64, ReflectValueRef::U64(v)) => {
                serializer.serialize_u64(*v)
            }
            (SomeIpPrimitiveTypeDefinition::I8, ReflectValueRef::I32(v)) => {
                match i8::try_from(*v) {
                    Ok(value) => serializer.serialize_i8(value),
                    Err(_) => Err(S::Error::custom("Error translating i32 value to i8")),
                }
            }
            (SomeIpPrimitiveTypeDefinition::I16, ReflectValueRef::I32(v)) => {
                match i16::try_from(*v) {
                    Ok(value) => serializer.serialize_i16(value),
                    Err(_) => Err(S::Error::custom("Error translating i32 value to i16")),
                }
            }
            (SomeIpPrimitiveTypeDefinition::I32, ReflectValueRef::I32(v)) => {
                serializer.serialize_i32(*v)
            }
            (SomeIpPrimitiveTypeDefinition::I64, ReflectValueRef::I64(v)) => {
                serializer.serialize_i64(*v)
            }
            (SomeIpPrimitiveTypeDefinition::F32, ReflectValueRef::F32(v)) => {
                serializer.serialize_f32(*v)
            }
            (SomeIpPrimitiveTypeDefinition::F64, ReflectValueRef::F64(v)) => {
                serializer.serialize_f64(*v)
            }
            // Add cases to handle enum to unsigned integer conversion
            (SomeIpPrimitiveTypeDefinition::U8, ReflectValueRef::Enum(_, enum_value)) => {
                match u8::try_from(*enum_value) {
                    Ok(value) => serializer.serialize_u8(value),
                    Err(_) => Err(S::Error::custom("Error translating enum value to u8")),
                }
            }
            (SomeIpPrimitiveTypeDefinition::U16, ReflectValueRef::Enum(_, enum_value)) => {
                match u16::try_from(*enum_value) {
                    Ok(value) => serializer.serialize_u16(value),
                    Err(_) => Err(S::Error::custom("Error translating enum value to u16")),
                }
            }
            (SomeIpPrimitiveTypeDefinition::U32, ReflectValueRef::Enum(_, enum_value)) => {
                match u32::try_from(*enum_value) {
                    Ok(value) => serializer.serialize_u32(value),
                    Err(_) => Err(S::Error::custom("Error translating enum value value to u32")),
                }
            }
            // Unexpected type, return error
            (expected_type, actual_type) => Err(S::Error::custom(format!(
                "Mismatch between SOME/IP and proto format - expected {}, actual {}",
                expected_type,
                actual_type.get_type()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use byteorder::BigEndian;
    use protobuf::descriptor::field_descriptor_proto::Label;
    use protobuf::{Enum, EnumFull};
    use serde::Serialize;

    use protobuf::reflect::ReflectValueRef;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};

    use crate::serializer::primitive::PrimitiveValueSeed;
    use crate::serializer::serializer::SomeIpSerdeSerializer;
    use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

    const SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::OneByte,
    };

    #[test]
    fn test_serialize_bool_succeeds() {
        let value = true;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::BOOLEAN;
        let proto_field_value = ReflectValueRef::Bool(value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), vec![1]);
    }

    #[test]
    fn test_serialize_u8_succeeds() {
        let value: u8 = 34;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U8;
        let proto_field_value = ReflectValueRef::U32(value.into());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), vec![value]);
    }

    #[test]
    fn test_serialize_u16_succeeds() {
        let value: u16 = 579;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U16;
        let proto_field_value = ReflectValueRef::U32(value.into());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_u32_succeeds() {
        let value: u32 = 20948;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U32;
        let proto_field_value = ReflectValueRef::U32(value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_u64_succeeds() {
        let value: u64 = 9634810;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U64;
        let proto_field_value = ReflectValueRef::U64(value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_i8_succeeds() {
        let value: i8 = -34;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I8;
        let proto_field_value = ReflectValueRef::I32(value.into());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), vec![value as u8]);
    }

    #[test]
    fn test_serialize_i16_succeeds() {
        let value: i16 = 579;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I16;
        let proto_field_value = ReflectValueRef::I32(value.into());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_i32_succeeds() {
        let value: i32 = 20948;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I32;
        let proto_field_value = ReflectValueRef::I32(value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_i64_succeeds() {
        let value: i64 = 9634810;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I64;
        let proto_field_value = ReflectValueRef::I64(value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_f32_succeeds() {
        let value: f32 = 8092.5f32;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::F32;
        let proto_field_value = ReflectValueRef::F32(value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_f64_succeeds() {
        let value: f64 = -380.28f64;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::F64;
        let proto_field_value = ReflectValueRef::F64(value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), value.to_be_bytes());
    }

    #[test]
    fn test_serialize_enum_to_u8_succeeds() {
        let enum_value = Label::LABEL_OPTIONAL.value(); // 1
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U8;
        let proto_field_value = ReflectValueRef::Enum(Label::enum_descriptor(), enum_value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), vec![enum_value as u8]);
    }

    #[test]
    fn test_serialize_enum_to_u16_succeeds() {
        let enum_value = Label::LABEL_REQUIRED.value(); // 2
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U16;
        let proto_field_value = ReflectValueRef::Enum(Label::enum_descriptor(), enum_value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), (enum_value as u16).to_be_bytes());
    }

    #[test]
    fn test_serialize_enum_to_u32_succeeds() {
        let enum_value = Label::LABEL_REPEATED.value(); // 3
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U32;
        let proto_field_value = ReflectValueRef::Enum(Label::enum_descriptor(), enum_value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        primitive_value_seed.serialize(&mut serializer).expect("Serialization must succeed");
        assert_eq!(serializer.get_serialized_bytes(), (enum_value as u32).to_be_bytes());
    }

    #[test]
    fn test_serialize_enum_to_u8_fails_on_overflow() {
        let enum_value = 300i32; // Too large for u8
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U8;
        let proto_field_value = ReflectValueRef::Enum(Label::enum_descriptor(), enum_value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        let result = primitive_value_seed.serialize(&mut serializer);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Error translating enum value to u8"
        );
    }

    #[test]
    fn test_serialize_enum_to_u16_fails_on_overflow() {
        let enum_value = 70000i32; // Too large for u16
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U16;
        let proto_field_value = ReflectValueRef::Enum(Label::enum_descriptor(), enum_value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        let result = primitive_value_seed.serialize(&mut serializer);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Error translating enum value to u16"
        );
    }

    #[test]
    fn test_serialize_enum_to_u32_fails_on_negative() {
        let enum_value = -1i32; // Negative, cannot be u32
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U32;
        let proto_field_value = ReflectValueRef::Enum(Label::enum_descriptor(), enum_value);
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        let result = primitive_value_seed.serialize(&mut serializer);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Error translating enum value value to u32"
        );
    }

    #[test]
    fn test_serialize_type_mismatch_enum_to_i8_fails() {
        let enum_value = 42i32;
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I8; // Mismatch: expecting I8 for SOME/IP
        let proto_field_value = ReflectValueRef::Enum(Label::enum_descriptor(), enum_value); // Actual is Enum
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, proto_field_value);

        let result = primitive_value_seed.serialize(&mut serializer);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Mismatch between SOME/IP and proto format - expected I8, actual google.protobuf.FieldDescriptorProto.Label"
        );
    }
}
