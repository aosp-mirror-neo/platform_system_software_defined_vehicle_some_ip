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

use serde::de::{DeserializeSeed, Visitor};
use serde::Deserializer;

use someip_serde_reflect::{SomeIpPrimitiveType, SomeIpPrimitiveTypeDefinition};

pub(crate) struct PrimitiveValueSeed<'lt> {
    primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
}

impl<'lt> PrimitiveValueSeed<'lt> {
    pub(crate) fn new(primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition) -> Self {
        Self { primitive_type_definition }
    }
}

impl<'de> DeserializeSeed<'de> for PrimitiveValueSeed<'_> {
    type Value = SomeIpPrimitiveType;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrimitiveValueVisitor;

        impl Visitor<'_> for PrimitiveValueVisitor {
            type Value = SomeIpPrimitiveType;
            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP primitive value")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::BOOLEAN(v))
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::U8(v))
            }

            fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::U16(v))
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::U32(v))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::U64(v))
            }

            fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::I8(v))
            }

            fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::I16(v))
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::I32(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::I64(v))
            }

            fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::F32(v))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SomeIpPrimitiveType::F64(v))
            }
        }

        let primitive_value_visitor = PrimitiveValueVisitor;
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
    use serde::de::DeserializeSeed;

    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::someip::primitive::PrimitiveValueSeed;
    use someip_serde_reflect::{SomeIpPrimitiveType, SomeIpPrimitiveTypeDefinition};

    #[test]
    fn test_deserialize_bool_succeeds() {
        let value: u8 = 0;
        let data = vec![value];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::BOOLEAN;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::BOOLEAN(false));
    }

    #[test]
    fn test_deserialize_u8_succeeds() {
        let value: u8 = 34;
        let data = vec![value];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U8;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::U8(value));
    }

    #[test]
    fn test_deserialize_u16_succeeds() {
        let value: u16 = 579;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U16;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::U16(value));
    }

    #[test]
    fn test_deserialize_u32_succeeds() {
        let value: u32 = 20948;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U32;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::U32(value));
    }

    #[test]
    fn test_deserialize_u64_succeeds() {
        let value: u64 = 9634810;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::U64;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::U64(value));
    }

    #[test]
    fn test_deserialize_i8_succeeds() {
        let value: i8 = 34;
        let data = vec![value as u8];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I8;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::I8(value));
    }

    #[test]
    fn test_deserialize_i16_succeeds() {
        let value: i16 = 579;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I16;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::I16(value));
    }

    #[test]
    fn test_deserialize_i32_succeeds() {
        let value: i32 = 20948;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I32;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::I32(value));
    }

    #[test]
    fn test_deserialize_i64_succeeds() {
        let value: i64 = 9634810;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::I64;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::I64(value));
    }

    #[test]
    fn test_deserialize_f32_succeeds() {
        let value: f32 = 8092.5f32;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::F32;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::F32(value));
    }

    #[test]
    fn test_deserialize_f64_succeeds() {
        let value: f64 = -380.28f64;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let primitive_type_definition = SomeIpPrimitiveTypeDefinition::F64;
        let primitive_value_seed = PrimitiveValueSeed::new(&primitive_type_definition);

        let result = primitive_value_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");
        assert_eq!(result, SomeIpPrimitiveType::F64(value));
    }
}
