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

use protobuf::reflect::{ReflectValueBox, RuntimeType};
use serde::de::{DeserializeSeed, Visitor};
use serde::Deserializer;

use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

pub(crate) struct PrimitiveValueSeed<'lt> {
    primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
    proto_runtime_type: &'lt RuntimeType,
}

impl<'lt> PrimitiveValueSeed<'lt> {
    pub(crate) fn new(
        primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
        proto_runtime_type: &'lt RuntimeType,
    ) -> Self {
        Self { primitive_type_definition, proto_runtime_type }
    }
}

impl<'de> DeserializeSeed<'de> for PrimitiveValueSeed<'_> {
    type Value = ReflectValueBox;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrimitiveValueVisitor<'lt> {
            proto_runtime_type: &'lt RuntimeType,
        }

        impl Visitor<'_> for PrimitiveValueVisitor<'_> {
            type Value = ReflectValueBox;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP primitive value")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if *self.proto_runtime_type != RuntimeType::Bool {
                    return Err(E::custom("Expected a boolean"));
                }
                Ok(ReflectValueBox::Bool(v))
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP u8 can only be translated to a proto u32, u64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::U32 => Ok(ReflectValueBox::U32(v.into())),
                    RuntimeType::U64 => Ok(ReflectValueBox::U64(v.into())),
                    RuntimeType::Enum(descriptor) => {
                        Ok(ReflectValueBox::Enum(descriptor.clone(), v.into()))
                    }
                    _ => Err(E::custom(
                        "SOME/IP U8 can only be deserialized to proto u32, u64 or enum",
                    )),
                }
            }

            fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP u16 can only be translated to a proto u32, u64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::U32 => Ok(ReflectValueBox::U32(v.into())),
                    RuntimeType::U64 => Ok(ReflectValueBox::U64(v.into())),
                    RuntimeType::Enum(descriptor) => {
                        Ok(ReflectValueBox::Enum(descriptor.clone(), v.into()))
                    }
                    _ => Err(E::custom(
                        "SOME/IP U16 can only be deserialized to proto u32, u64 or enum",
                    )),
                }
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP u32 can only be translated to a proto u32, u64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::U32 => Ok(ReflectValueBox::U32(v)),
                    RuntimeType::U64 => Ok(ReflectValueBox::U64(v.into())),
                    RuntimeType::Enum(descriptor) => i32::try_from(v)
                        .map(|val_i32| ReflectValueBox::Enum(descriptor.clone(), val_i32))
                        .map_err(|_| E::custom(format!("Enum value max value is {}", i32::MAX))),
                    _ => Err(E::custom(
                        "SOME/IP U32 can only be deserialized to proto u32, u64 or enum",
                    )),
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP u64 can only be translated to a proto u64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::U64 => Ok(ReflectValueBox::U64(v)),
                    RuntimeType::Enum(descriptor) => i32::try_from(v)
                        .map(|val_i32| ReflectValueBox::Enum(descriptor.clone(), val_i32))
                        .map_err(|_| E::custom(format!("Enum value max value is {}", i32::MAX))),
                    _ => {
                        Err(E::custom("SOME/IP U64 can only be deserialized to proto u64 or enum"))
                    }
                }
            }

            fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP i8 can only be translated to a proto i32, i64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::I32 => Ok(ReflectValueBox::I32(v.into())),
                    RuntimeType::I64 => Ok(ReflectValueBox::I64(v.into())),
                    RuntimeType::Enum(descriptor) => {
                        Ok(ReflectValueBox::Enum(descriptor.clone(), v.into()))
                    }
                    _ => Err(E::custom(
                        "SOME/IP I8 can only be deserialized to proto i32, i64 or enum",
                    )),
                }
            }

            fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP i16 can only be translated to a proto i32, i64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::I32 => Ok(ReflectValueBox::I32(v.into())),
                    RuntimeType::I64 => Ok(ReflectValueBox::I64(v.into())),
                    RuntimeType::Enum(descriptor) => {
                        Ok(ReflectValueBox::Enum(descriptor.clone(), v.into()))
                    }
                    _ => Err(E::custom(
                        "SOME/IP I16 can only be deserialized to proto i32, i64 or enum",
                    )),
                }
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP i32 can only be translated to a proto i32, i64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::I32 => Ok(ReflectValueBox::I32(v)),
                    RuntimeType::I64 => Ok(ReflectValueBox::I64(v.into())),
                    RuntimeType::Enum(descriptor) => {
                        Ok(ReflectValueBox::Enum(descriptor.clone(), v))
                    }
                    _ => Err(E::custom(
                        "SOME/IP I32 can only be deserialized to proto i32, i64 or enum",
                    )),
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP i64 can only be translated to a proto i64 or an enum
                match self.proto_runtime_type {
                    RuntimeType::I64 => Ok(ReflectValueBox::I64(v)),
                    RuntimeType::Enum(descriptor) => i32::try_from(v)
                        .map(|val_i32| ReflectValueBox::Enum(descriptor.clone(), val_i32))
                        .map_err(|_| E::custom(format!("Enum value max value is {}", i32::MAX))),
                    _ => {
                        Err(E::custom("SOME/IP I64 can only be deserialized to proto i64 or enum"))
                    }
                }
            }

            fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // SOME/IP f32 can only be translated to a proto f32 or f64
                match self.proto_runtime_type {
                    RuntimeType::F32 => Ok(ReflectValueBox::F32(v)),
                    RuntimeType::F64 => Ok(ReflectValueBox::F64(v.into())),
                    _ => Err(E::custom("SOME/IP F32 can only be deserialized to proto f32 or f64")),
                }
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if *self.proto_runtime_type != RuntimeType::F64 {
                    return Err(E::custom("Expected f64"));
                }
                Ok(ReflectValueBox::F64(v))
            }
        }

        let primitive_value_visitor =
            PrimitiveValueVisitor { proto_runtime_type: self.proto_runtime_type };

        match self.primitive_type_definition {
            SomeIpPrimitiveTypeDefinition::BOOLEAN => {
                deserializer.deserialize_bool(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::U8 => {
                deserializer.deserialize_u8(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::U16 => {
                deserializer.deserialize_u16(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::U32 => {
                deserializer.deserialize_u32(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::U64 => {
                deserializer.deserialize_u64(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::I8 => {
                deserializer.deserialize_i8(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::I16 => {
                deserializer.deserialize_i16(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::I32 => {
                deserializer.deserialize_i32(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::I64 => {
                deserializer.deserialize_i64(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::F32 => {
                deserializer.deserialize_f32(primitive_value_visitor)
            }
            SomeIpPrimitiveTypeDefinition::F64 => {
                deserializer.deserialize_f64(primitive_value_visitor)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use byteorder::BigEndian;
    use protobuf::EnumFull;
    use serde::de::DeserializeSeed;
    use someip_serde_test_protos::messages::TestEnum;

    use protobuf::reflect::{ReflectValueBox, RuntimeType};

    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::protobuf::primitive::PrimitiveValueSeed;
    use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

    #[test]
    fn test_deserialize_bool_succeeds() {
        let value: u8 = 0;
        let data = vec![value];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::BOOLEAN;
        let proto_runtime_type = RuntimeType::Bool;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::Bool(false));
    }

    #[test]
    fn test_deserialize_u8_succeeds() {
        let value: u8 = 34;
        let data = vec![value];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U8;
        let proto_runtime_type = RuntimeType::U32;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::U32(value.into()));
    }

    #[test]
    fn test_deserialize_u16_succeeds() {
        let value: u16 = 579;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U16;
        let proto_runtime_type = RuntimeType::U64;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::U64(value.into()));
    }

    #[test]
    fn test_deserialize_u32_succeeds() {
        let value: u32 = 20948;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U32;
        let proto_runtime_type = RuntimeType::U32;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::U32(value));
    }

    #[test]
    fn test_deserialize_u64_succeeds() {
        let value: u64 = 9634810;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U64;
        let proto_runtime_type = RuntimeType::U64;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::U64(value));
    }

    #[test]
    fn test_deserialize_i8_succeeds() {
        let value: i8 = 34;
        let data = vec![value as u8];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I8;
        let proto_runtime_type = RuntimeType::I64;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::I64(value.into()));
    }

    #[test]
    fn test_deserialize_i16_succeeds() {
        let value: i16 = 579;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I16;
        let proto_runtime_type = RuntimeType::I32;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::I32(value.into()));
    }

    #[test]
    fn test_deserialize_i32_succeeds() {
        let value: i32 = 20948;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I32;
        let proto_runtime_type = RuntimeType::I32;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::I32(value));
    }

    #[test]
    fn test_deserialize_i64_succeeds() {
        let value: i64 = 9634810;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I64;
        let proto_runtime_type = RuntimeType::I64;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::I64(value));
    }

    #[test]
    fn test_deserialize_f32_succeeds() {
        let value: f32 = 8092.5f32;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::F32;
        let proto_runtime_type = RuntimeType::F32;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::F32(value));
    }

    #[test]
    fn test_deserialize_f64_succeeds() {
        let value: f64 = -380.28f64;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::F64;
        let proto_runtime_type = RuntimeType::F64;
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, ReflectValueBox::F64(value));
    }

    #[test]
    fn test_deserialize_u8_to_enum_succeeds() {
        let value: u8 = TestEnum::SECOND_VALUE as u8;
        let data = vec![value];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U8;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::SECOND_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_u16_to_enum_succeeds() {
        let value: u16 = TestEnum::THIRD_VALUE as u16;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U16;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::THIRD_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_u32_to_enum_succeeds() {
        let value: u32 = TestEnum::FIRST_VALUE as u32;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U32;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::FIRST_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_u32_to_enum_value_out_of_i32_range_fails() {
        let value: u32 = i32::MAX as u32 + 1; // Value that cannot fit in i32
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U32;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        assert!(primitive_value_seed.deserialize(&mut deserializer).is_err());
    }

    #[test]
    fn test_deserialize_u64_to_enum_succeeds() {
        let value: u64 = TestEnum::SECOND_VALUE as u64;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U64;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::SECOND_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_u64_to_enum_value_out_of_i32_range_fails() {
        let value: u64 = i32::MAX as u64 + 1; // Value that cannot fit in i32
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U64;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed.deserialize(&mut deserializer);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_i8_to_enum_succeeds() {
        let value: i8 = TestEnum::FIRST_VALUE as i8;
        let data = vec![value as u8]; // i8 to u8 for byte stream
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I8;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::FIRST_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_i16_to_enum_succeeds() {
        let value: i16 = TestEnum::SECOND_VALUE as i16;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I16;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::SECOND_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_i32_to_enum_succeeds() {
        let value: i32 = TestEnum::THIRD_VALUE as i32;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I32;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::THIRD_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_i64_to_enum_succeeds() {
        let value: i64 = TestEnum::FIRST_VALUE as i64;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I64;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(
            result,
            ReflectValueBox::Enum(TestEnum::enum_descriptor(), TestEnum::FIRST_VALUE as i32)
        );
    }

    #[test]
    fn test_deserialize_i64_to_enum_value_out_of_i32_range_fails() {
        let value: i64 = i32::MAX as i64 + 1; // Value that cannot fit in i32
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I64;
        let proto_runtime_type = RuntimeType::Enum(TestEnum::enum_descriptor());
        let primitive_value_seed =
            PrimitiveValueSeed::new(&primitive_type_definition, &proto_runtime_type);

        assert!(primitive_value_seed.deserialize(&mut deserializer).is_err());
    }
}
