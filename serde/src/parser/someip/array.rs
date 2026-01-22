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

use byteorder::{BigEndian, ByteOrder, LittleEndian};

use serde::de::{DeserializeSeed, Error, SeqAccess, Visitor};
use serde::Deserializer;

use someip_config::serde_config::ffi::Endianness;

use crate::parser::common::LengthFieldDeserializeSeed;
use crate::parser::someip::complex::StructValueSeed;
use crate::parser::someip::string::StringValueSeed;
use someip_serde_reflect::{
    SomeIpArrayType, SomeIpArrayTypeDefinition, SomeIpMessageData, SomeIpMessageTypeDefinition,
    SomeIpPrimitiveType, SomeIpPrimitiveTypeDefinition,
};

pub(crate) struct ArrayValueDeserializeSeed<'lt> {
    array_type_definition: &'lt SomeIpArrayTypeDefinition,
    endianness: Endianness,
}

impl<'lt> ArrayValueDeserializeSeed<'lt> {
    pub(crate) fn new(
        array_type_definition: &'lt SomeIpArrayTypeDefinition,
        endianness: Endianness,
    ) -> Self {
        Self { array_type_definition, endianness }
    }
}

impl<'de> DeserializeSeed<'de> for ArrayValueDeserializeSeed<'_> {
    type Value = SomeIpArrayType;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayValueVisitor<'lt> {
            array_type_definition: &'lt SomeIpArrayTypeDefinition,
            endianness: Endianness,
        }

        impl<'lt> ArrayValueVisitor<'lt> {
            fn new(
                array_type_definition: &'lt SomeIpArrayTypeDefinition,
                endianness: Endianness,
            ) -> Self {
                ArrayValueVisitor { array_type_definition, endianness }
            }
        }

        impl<'de> Visitor<'de> for ArrayValueVisitor<'_> {
            type Value = SomeIpArrayType;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP array")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                // Get array size
                let array_size = {
                    if let Some(size) = self.array_type_definition.fixed_length {
                        size
                    } else {
                        let optional_size =
                            seq.next_element_seed(LengthFieldDeserializeSeed::new(
                                &self.array_type_definition.length_field_size,
                            ))?;
                        if let Some(size) = optional_size {
                            size
                        } else {
                            return Err(A::Error::custom("SOME/IP array has unknown size"));
                        }
                    }
                };
                match &*self.array_type_definition.array_type {
                    SomeIpMessageTypeDefinition::PRIMITIVE(primitive_type_definition) => {
                        let Some(primitive_array) =
                            seq.next_element_seed(PrimitiveArrayDeserializeSeed::new(
                                primitive_type_definition,
                                array_size,
                                self.endianness,
                            ))?
                        else {
                            return Err(A::Error::custom(
                                "Unable to deserialize primitive type array",
                            ));
                        };
                        Ok(primitive_array)
                    }
                    message_type_definition => {
                        let Some(array) =
                            seq.next_element_seed(GeneralArrayDeserializeSeed::new(
                                message_type_definition,
                                array_size,
                                self.endianness,
                            ))?
                        else {
                            return Err(A::Error::custom("Unable to deserialize array"));
                        };
                        Ok(array)
                    }
                }
            }
        }
        deserializer
            .deserialize_seq(ArrayValueVisitor::new(self.array_type_definition, self.endianness))
    }
}

struct PrimitiveArrayDeserializeSeed<'lt> {
    primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
    size: usize,
    endianness: Endianness,
}

impl<'lt> PrimitiveArrayDeserializeSeed<'lt> {
    fn new(
        primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
        size: usize,
        endianness: Endianness,
    ) -> Self {
        PrimitiveArrayDeserializeSeed { primitive_type_definition, size, endianness }
    }
}

impl<'de> DeserializeSeed<'de> for PrimitiveArrayDeserializeSeed<'_> {
    type Value = SomeIpArrayType;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrimitiveArrayVisitor<'lt> {
            primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
            endianness: Endianness,
        }

        impl<'lt> PrimitiveArrayVisitor<'lt> {
            fn new(
                primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
                endianness: Endianness,
            ) -> Self {
                PrimitiveArrayVisitor { primitive_type_definition, endianness }
            }
        }

        impl Visitor<'_> for PrimitiveArrayVisitor<'_> {
            type Value = SomeIpArrayType;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP primitive array")
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let mut array_data = Vec::with_capacity(value.len());
                match self.primitive_type_definition {
                    SomeIpPrimitiveTypeDefinition::BOOLEAN => {
                        for element in value {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::BOOLEAN(element != 0),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::U8 => {
                        for element in value {
                            array_data.push(SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U8(
                                element,
                            )));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::U16 => {
                        if !value.len().is_multiple_of(2) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0; value.len() / 2];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_u16_into(&value, &mut values);
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_u16_into(&value, &mut values);
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::U16(element),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::U32 => {
                        if !value.len().is_multiple_of(4) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0; value.len() / 4];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_u32_into(&value, &mut values);
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_u32_into(&value, &mut values);
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::U32(element),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::U64 => {
                        if !value.len().is_multiple_of(8) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0; value.len() / 8];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_u64_into(&value, &mut values);
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_u64_into(&value, &mut values);
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::U64(element),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::I8 => {
                        for element in value {
                            array_data.push(SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I8(
                                element as i8,
                            )));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::I16 => {
                        if !value.len().is_multiple_of(2) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0; value.len() / 2];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_i16_into(&value, &mut values);
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_i16_into(&value, &mut values);
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::I16(element),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::I32 => {
                        if !value.len().is_multiple_of(4) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0; value.len() / 4];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_i32_into(&value, &mut values);
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_i32_into(&value, &mut values);
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::I32(element),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::I64 => {
                        if !value.len().is_multiple_of(8) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0; value.len() / 8];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_i64_into(&value, &mut values);
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_i64_into(&value, &mut values);
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::I64(element),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::F32 => {
                        if !value.len().is_multiple_of(4) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0f32; value.len() / 4];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_f32_into(&value, values.as_mut_slice());
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_f32_into(&value, values.as_mut_slice());
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::F32(element),
                            ));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::F64 => {
                        if !value.len().is_multiple_of(8) {
                            return Err(E::custom("Internal error"));
                        }
                        let mut values = vec![0f64; value.len() / 8];
                        match self.endianness {
                            Endianness::BigEndian => {
                                BigEndian::read_f64_into(&value, values.as_mut_slice());
                            }
                            Endianness::LittleEndian => {
                                LittleEndian::read_f64_into(&value, values.as_mut_slice());
                            }
                            _ => {
                                return Err(E::custom("Internal error"));
                            }
                        }
                        for element in values {
                            array_data.push(SomeIpMessageData::PRIMITIVE(
                                SomeIpPrimitiveType::F64(element),
                            ));
                        }
                    }
                }
                Ok(SomeIpArrayType { array_data })
            }
        }
        if self.size == 0 {
            return Ok(SomeIpArrayType { array_data: Vec::new() });
        }
        let bytes_to_read = match self.primitive_type_definition {
            SomeIpPrimitiveTypeDefinition::BOOLEAN
            | SomeIpPrimitiveTypeDefinition::U8
            | SomeIpPrimitiveTypeDefinition::I8 => self.size,
            SomeIpPrimitiveTypeDefinition::U16 | SomeIpPrimitiveTypeDefinition::I16 => {
                (self.size / 2) * 2
            }
            SomeIpPrimitiveTypeDefinition::U32
            | SomeIpPrimitiveTypeDefinition::I32
            | SomeIpPrimitiveTypeDefinition::F32 => (self.size / 4) * 4,
            SomeIpPrimitiveTypeDefinition::U64
            | SomeIpPrimitiveTypeDefinition::I64
            | SomeIpPrimitiveTypeDefinition::F64 => (self.size / 8) * 8,
        };
        deserializer.deserialize_tuple(
            bytes_to_read,
            PrimitiveArrayVisitor::new(self.primitive_type_definition, self.endianness),
        )
    }
}

struct GeneralArrayDeserializeSeed<'lt> {
    type_definition: &'lt SomeIpMessageTypeDefinition,
    size: usize,
    endianness: Endianness,
}

impl<'lt> GeneralArrayDeserializeSeed<'lt> {
    fn new(
        type_definition: &'lt SomeIpMessageTypeDefinition,
        size: usize,
        endianness: Endianness,
    ) -> Self {
        GeneralArrayDeserializeSeed { type_definition, size, endianness }
    }
}

impl<'de> DeserializeSeed<'de> for GeneralArrayDeserializeSeed<'_> {
    type Value = SomeIpArrayType;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GeneralArrayVisitor<'lt> {
            type_definition: &'lt SomeIpMessageTypeDefinition,
            endianness: Endianness,
        }

        impl<'lt> GeneralArrayVisitor<'lt> {
            fn new(
                type_definition: &'lt SomeIpMessageTypeDefinition,
                endianness: Endianness,
            ) -> Self {
                GeneralArrayVisitor { type_definition, endianness }
            }
        }

        impl<'de> Visitor<'de> for GeneralArrayVisitor<'_> {
            type Value = SomeIpArrayType;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP primitive array")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut array_data = Vec::new();
                match self.type_definition {
                    SomeIpMessageTypeDefinition::STRING(string_type_definition) => {
                        while let Some(next_element) =
                            seq.next_element_seed(StringValueSeed::new(string_type_definition))?
                        {
                            array_data.push(SomeIpMessageData::STRING(next_element));
                        }
                    }
                    SomeIpMessageTypeDefinition::ARRAY(array_type_definition) => {
                        while let Some(next_element) = seq.next_element_seed(
                            ArrayValueDeserializeSeed::new(array_type_definition, self.endianness),
                        )? {
                            array_data.push(SomeIpMessageData::ARRAY(next_element));
                        }
                    }
                    SomeIpMessageTypeDefinition::STRUCT(struct_type_definition) => {
                        while let Some(next_element) = seq.next_element_seed(
                            StructValueSeed::new(struct_type_definition, self.endianness),
                        )? {
                            array_data.push(SomeIpMessageData::STRUCT(next_element));
                        }
                    }
                    _ => {
                        // Impossible since we checked in ArrayValueVisitor
                        return Err(A::Error::custom("Internal error"));
                    }
                }
                Ok(SomeIpArrayType { array_data })
            }
        }
        // We use deserialize_tuple is for cases where it is simple to deserialize the array elements inside the visitor (primitive data types),
        // whereas deserialize_tuple_struct is for cases where the array element type is more complex, we use it to drive the deserializer to call
        // the other DeserializeSeeds we have. TODO(b/403246093)
        deserializer.deserialize_tuple_struct(
            "",
            self.size,
            GeneralArrayVisitor::new(self.type_definition, self.endianness),
        )
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{BigEndian, LittleEndian};
    use serde::de::DeserializeSeed;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, StringType};

    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::someip::array::ArrayValueDeserializeSeed;
    use someip_serde_reflect::{
        SomeIpArrayType, SomeIpArrayTypeDefinition, SomeIpMessageData, SomeIpMessageTypeDefinition,
        SomeIpPrimitiveType, SomeIpPrimitiveTypeDefinition, SomeIpStringTypeDefinition,
        SomeIpStructFieldType, SomeIpStructFieldTypeDefinition, SomeIpStructType,
        SomeIpStructTypeDefinition,
    };

    const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];

    #[test]
    fn test_u8_array_deserialize_succeeds() {
        let payload = vec![3, 12, 42, 23];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
        };
        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U8(12)),
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U8(42)),
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U8(23)),
                ]
            }
        );
    }

    #[test]
    fn test_i16_array_deserialize_succeeds() {
        let mut payload = vec![6];
        let data: Vec<i16> = vec![-12, 43, -78];
        for element in &data {
            payload.extend_from_slice(&element.to_be_bytes());
        }
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I16,
            )),
        };
        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I16(data[0])),
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I16(data[1])),
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I16(data[2])),
                ]
            }
        );
    }

    #[test]
    fn test_u32_array_deserialize_succeeds() {
        let mut payload = vec![15]; // actually 12 bytes, but visitor should interpret it correctly
        let data: Vec<u32> = vec![6373, 1290, 3529];
        for element in &data {
            payload.extend_from_slice(&element.to_be_bytes());
        }
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U32,
            )),
        };
        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U32(data[0])),
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U32(data[1])),
                    SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U32(data[2])),
                ]
            }
        );
    }

    #[test]
    fn test_string_array_deserialize_succeeds() {
        let mut payload_data: Vec<u8> = Vec::new();
        let data =
            vec![String::from("Hello"), String::from("Test"), String::from("Another String")];
        for element in &data {
            payload_data.push((element.len() + BOM_UTF8.len() + 1) as u8);
            payload_data.extend_from_slice(BOM_UTF8);
            payload_data.extend_from_slice(element.as_bytes());
            payload_data.push(0x00);
        }
        let mut payload = Vec::new();
        println!("Payload length is {}", payload_data.len());
        payload.push(payload_data.len() as u8);
        payload.extend_from_slice(&payload_data);
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                length_field_size: LenFieldSize::OneByte,
                bom: StringType::UTF_8,
                fixed_length: None,
            })),
        };

        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::STRING(data[0].clone()),
                    SomeIpMessageData::STRING(data[1].clone()),
                    SomeIpMessageData::STRING(data[2].clone()),
                ]
            }
        );
    }

    #[test]
    fn test_2d_array_deserialize_succeeds() {
        let mut payload = Vec::new();
        let data: Vec<u16> = vec![12, 43, 78];
        let outer_array_length = 3;
        payload.extend_from_slice(&24u16.to_le_bytes());
        for _i in 0..outer_array_length {
            payload.extend_from_slice(&6u16.to_le_bytes());
            for element in &data {
                payload.extend_from_slice(&element.to_le_bytes());
            }
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::TwoBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::TwoBytes,
                array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                    SomeIpPrimitiveTypeDefinition::U16,
                )),
            })),
        };
        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::LittleEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::ARRAY(SomeIpArrayType {
                        array_data: vec![
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[0])),
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[1])),
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[2])),
                        ]
                    }),
                    SomeIpMessageData::ARRAY(SomeIpArrayType {
                        array_data: vec![
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[0])),
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[1])),
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[2])),
                        ]
                    }),
                    SomeIpMessageData::ARRAY(SomeIpArrayType {
                        array_data: vec![
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[0])),
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[1])),
                            SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(data[2])),
                        ]
                    })
                ]
            }
        );
    }

    #[test]
    fn test_struct_array_deserialize_succeeds() {
        let mut payload_data: Vec<u8> = Vec::new();
        let struct_vals = vec![(3i32, 5u16), (-94i32, 60u16), (-304i32, 3u16)];

        for (a, b) in &struct_vals {
            payload_data.extend_from_slice(&a.to_be_bytes());
            payload_data.extend_from_slice(&b.to_be_bytes());
        }
        let mut payload = Vec::new();
        payload.extend_from_slice(&(payload_data.len() as u16).to_be_bytes());
        payload.extend_from_slice(&payload_data);
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::TwoBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "Test".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: vec![
                    SomeIpStructFieldTypeDefinition {
                        data_tag: Option::None,
                        field_name: "i32".to_string(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::I32,
                        ),
                    },
                    SomeIpStructFieldTypeDefinition {
                        data_tag: Option::None,
                        field_name: "u16".to_string(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U16,
                        ),
                    },
                ],
            })),
        };

        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::STRUCT(SomeIpStructType {
                        fields: vec![
                            SomeIpStructFieldType {
                                field_name: "i32".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I32(
                                    struct_vals[0].0
                                ))
                            },
                            SomeIpStructFieldType {
                                field_name: "u16".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(
                                    struct_vals[0].1
                                ))
                            },
                        ]
                    }),
                    SomeIpMessageData::STRUCT(SomeIpStructType {
                        fields: vec![
                            SomeIpStructFieldType {
                                field_name: "i32".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I32(
                                    struct_vals[1].0
                                ))
                            },
                            SomeIpStructFieldType {
                                field_name: "u16".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(
                                    struct_vals[1].1
                                ))
                            },
                        ]
                    }),
                    SomeIpMessageData::STRUCT(SomeIpStructType {
                        fields: vec![
                            SomeIpStructFieldType {
                                field_name: "i32".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I32(
                                    struct_vals[2].0
                                ))
                            },
                            SomeIpStructFieldType {
                                field_name: "u16".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(
                                    struct_vals[2].1
                                ))
                            },
                        ]
                    }),
                ]
            }
        );
    }

    #[test]
    fn test_struct_array_deserialize_little_endian_succeeds() {
        let mut payload_data: Vec<u8> = Vec::new();
        let struct_vals = vec![(3i32, 5u16), (-94i32, 60u16), (-304i32, 3u16)];

        for (a, b) in &struct_vals {
            payload_data.extend_from_slice(&a.to_le_bytes()); // Little Endian
            payload_data.extend_from_slice(&b.to_le_bytes()); // Little Endian
        }
        let mut payload = Vec::new();
        payload.extend_from_slice(&(payload_data.len() as u16).to_le_bytes()); // Little Endian length
        payload.extend_from_slice(&payload_data);
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::TwoBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "Test".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: vec![
                    SomeIpStructFieldTypeDefinition {
                        data_tag: Option::None,
                        field_name: "i32".to_string(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::I32,
                        ),
                    },
                    SomeIpStructFieldTypeDefinition {
                        data_tag: Option::None,
                        field_name: "u16".to_string(),
                        field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U16,
                        ),
                    },
                ],
            })),
        };

        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::LittleEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::STRUCT(SomeIpStructType {
                        fields: vec![
                            SomeIpStructFieldType {
                                field_name: "i32".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I32(
                                    struct_vals[0].0
                                ))
                            },
                            SomeIpStructFieldType {
                                field_name: "u16".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(
                                    struct_vals[0].1
                                ))
                            },
                        ]
                    }),
                    SomeIpMessageData::STRUCT(SomeIpStructType {
                        fields: vec![
                            SomeIpStructFieldType {
                                field_name: "i32".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I32(
                                    struct_vals[1].0
                                ))
                            },
                            SomeIpStructFieldType {
                                field_name: "u16".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(
                                    struct_vals[1].1
                                ))
                            },
                        ]
                    }),
                    SomeIpMessageData::STRUCT(SomeIpStructType {
                        fields: vec![
                            SomeIpStructFieldType {
                                field_name: "i32".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I32(
                                    struct_vals[2].0
                                ))
                            },
                            SomeIpStructFieldType {
                                field_name: "u16".to_string(),
                                field_data: SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U16(
                                    struct_vals[2].1
                                ))
                            },
                        ]
                    }),
                ]
            }
        );
    }

    #[test]
    fn test_string_array_deserialize_little_endian_succeeds() {
        let mut payload_data: Vec<u8> = Vec::new();
        let data =
            vec![String::from("Hello"), String::from("Test"), String::from("Another String")];
        for element in &data {
            // Length field for string is OneByte, so endianness doesn't apply here.
            payload_data.push((element.len() + BOM_UTF8.len() + 1) as u8);
            payload_data.extend_from_slice(BOM_UTF8);
            payload_data.extend_from_slice(element.as_bytes());
            payload_data.push(0x00);
        }
        let mut payload = Vec::new();
        // Length field for array is OneByte, so endianness doesn't apply here for payload generation.
        payload.push(payload_data.len() as u8);
        payload.extend_from_slice(&payload_data);
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                length_field_size: LenFieldSize::OneByte,
                bom: StringType::UTF_8,
                fixed_length: None,
            })),
        };

        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::LittleEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(
            result,
            SomeIpArrayType {
                array_data: vec![
                    SomeIpMessageData::STRING(data[0].clone()),
                    SomeIpMessageData::STRING(data[1].clone()),
                    SomeIpMessageData::STRING(data[2].clone()),
                ]
            }
        );
    }

    #[test]
    fn test_fixed_length_boolean_array_deserialize_succeeds() {
        let data_bool: Vec<bool> = vec![true, false, true, true, false];
        let payload: Vec<u8> = data_bool.iter().map(|&b| if b { 0x01 } else { 0x00 }).collect();
        let num_elements = data_bool.len();

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload); // Endianness doesn't matter for bool/u8 elements

        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(num_elements), // num_elements * 1 byte_per_element
            length_field_size: LenFieldSize::ZeroBytes, // Ignored for fixed_length
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::BOOLEAN,
            )),
        };
        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);

        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let expected_array_data: Vec<SomeIpMessageData> = data_bool
            .iter()
            .map(|&b| SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::BOOLEAN(b)))
            .collect();

        assert_eq!(result, SomeIpArrayType { array_data: expected_array_data });
    }

    #[test]
    fn test_fixed_length_u64_array_deserialize_succeeds() {
        let data_u64: Vec<u64> = vec![10, 20, u64::MAX];
        let num_elements = data_u64.len();

        // Big Endian
        let mut payload_be = Vec::new();
        for &val in &data_u64 {
            payload_be.extend_from_slice(&val.to_be_bytes());
        }
        let mut deserializer_be = SomeIpSerdeDeserializer::<BigEndian>::new(&payload_be);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(num_elements * 8), // num_elements * 8 bytes_per_element
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U64,
            )),
        };
        let array_value_seed_be =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);
        let result_be = array_value_seed_be
            .deserialize(&mut deserializer_be)
            .expect("BE Deserialization must succeed");
        let expected_array_data: Vec<SomeIpMessageData> = data_u64
            .iter()
            .map(|&val| SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::U64(val)))
            .collect();
        assert_eq!(result_be, SomeIpArrayType { array_data: expected_array_data.clone() });

        // Little Endian
        let mut payload_le = Vec::new();
        for &val in &data_u64 {
            payload_le.extend_from_slice(&val.to_le_bytes());
        }
        let mut deserializer_le = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload_le);
        let array_value_seed_le =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::LittleEndian);
        let result_le = array_value_seed_le
            .deserialize(&mut deserializer_le)
            .expect("LE Deserialization must succeed");
        assert_eq!(result_le, SomeIpArrayType { array_data: expected_array_data });
    }

    #[test]
    fn test_fixed_length_i8_array_deserialize_succeeds() {
        let data_i8: Vec<i8> = vec![10, -20, 0, i8::MAX, i8::MIN];
        let payload: Vec<u8> = data_i8.iter().map(|&val| val as u8).collect();
        let num_elements = data_i8.len();

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(num_elements), // num_elements * 1 byte_per_element
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I8,
            )),
        };
        let array_value_seed =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);
        let result =
            array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");
        let expected_array_data: Vec<SomeIpMessageData> = data_i8
            .iter()
            .map(|&val| SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I8(val)))
            .collect();
        assert_eq!(result, SomeIpArrayType { array_data: expected_array_data });
    }

    #[test]
    fn test_fixed_length_i32_array_deserialize_succeeds() {
        let data_i32: Vec<i32> = vec![100, -200, 0, i32::MAX, i32::MIN];
        let num_elements = data_i32.len();

        // Big Endian
        let mut payload_be = Vec::new();
        for &val in &data_i32 {
            payload_be.extend_from_slice(&val.to_be_bytes());
        }
        let mut deserializer_be = SomeIpSerdeDeserializer::<BigEndian>::new(&payload_be);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(num_elements * 4), // num_elements * 4 bytes_per_element
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I32,
            )),
        };
        let array_value_seed_be =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);
        let result_be = array_value_seed_be
            .deserialize(&mut deserializer_be)
            .expect("BE Deserialization must succeed");
        let expected_array_data: Vec<SomeIpMessageData> = data_i32
            .iter()
            .map(|&val| SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I32(val)))
            .collect();
        assert_eq!(result_be, SomeIpArrayType { array_data: expected_array_data.clone() });

        // Little Endian
        let mut payload_le = Vec::new();
        for &val in &data_i32 {
            payload_le.extend_from_slice(&val.to_le_bytes());
        }
        let mut deserializer_le = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload_le);
        let array_value_seed_le =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::LittleEndian);
        let result_le = array_value_seed_le
            .deserialize(&mut deserializer_le)
            .expect("LE Deserialization must succeed");
        assert_eq!(result_le, SomeIpArrayType { array_data: expected_array_data });
    }

    #[test]
    fn test_fixed_length_i64_array_deserialize_succeeds() {
        let data_i64: Vec<i64> = vec![1000, -2000, 0, i64::MAX, i64::MIN];
        let num_elements = data_i64.len();

        // Big Endian
        let mut payload_be = Vec::new();
        for &val in &data_i64 {
            payload_be.extend_from_slice(&val.to_be_bytes());
        }
        let mut deserializer_be = SomeIpSerdeDeserializer::<BigEndian>::new(&payload_be);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(num_elements * 8), // num_elements * 8 bytes_per_element
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I64,
            )),
        };
        let array_value_seed_be =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);
        let result_be = array_value_seed_be
            .deserialize(&mut deserializer_be)
            .expect("BE Deserialization must succeed");
        let expected_array_data: Vec<SomeIpMessageData> = data_i64
            .iter()
            .map(|&val| SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::I64(val)))
            .collect();
        assert_eq!(result_be, SomeIpArrayType { array_data: expected_array_data.clone() });

        // Little Endian
        let mut payload_le = Vec::new();
        for &val in &data_i64 {
            payload_le.extend_from_slice(&val.to_le_bytes());
        }
        let mut deserializer_le = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload_le);
        let array_value_seed_le =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::LittleEndian);
        let result_le = array_value_seed_le
            .deserialize(&mut deserializer_le)
            .expect("LE Deserialization must succeed");
        assert_eq!(result_le, SomeIpArrayType { array_data: expected_array_data });
    }

    #[test]
    fn test_fixed_length_f64_array_deserialize_succeeds() {
        let data_f64: Vec<f64> = vec![10.5, -20.25, 0.0, f64::MAX, f64::MIN, f64::NAN];
        let num_elements = data_f64.len();

        // Big Endian
        let mut payload_be = Vec::new();
        for &val in &data_f64 {
            payload_be.extend_from_slice(&val.to_be_bytes());
        }
        let mut deserializer_be = SomeIpSerdeDeserializer::<BigEndian>::new(&payload_be);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(num_elements * 8), // num_elements * 8 bytes_per_element
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::F64,
            )),
        };
        let array_value_seed_be =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::BigEndian);
        let result_be = array_value_seed_be
            .deserialize(&mut deserializer_be)
            .expect("BE Deserialization must succeed");

        assert_eq!(result_be.array_data.len(), num_elements);
        for (i, val) in data_f64.iter().enumerate() {
            match result_be.array_data[i] {
                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::F64(res_val)) => {
                    if val.is_nan() {
                        assert!(res_val.is_nan());
                    } else {
                        assert_eq!(res_val, *val);
                    }
                }
                _ => panic!("Unexpected element type in result array"),
            }
        }

        // Little Endian
        let mut payload_le = Vec::new();
        for &val in &data_f64 {
            payload_le.extend_from_slice(&val.to_le_bytes());
        }
        let mut deserializer_le = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload_le);
        let array_value_seed_le =
            ArrayValueDeserializeSeed::new(&array_type_definition, Endianness::LittleEndian);
        let result_le = array_value_seed_le
            .deserialize(&mut deserializer_le)
            .expect("LE Deserialization must succeed");

        assert_eq!(result_le.array_data.len(), num_elements);
        for (i, val) in data_f64.iter().enumerate() {
            match result_le.array_data[i] {
                SomeIpMessageData::PRIMITIVE(SomeIpPrimitiveType::F64(res_val)) => {
                    if val.is_nan() {
                        assert!(res_val.is_nan());
                    } else {
                        assert_eq!(res_val, *val);
                    }
                }
                _ => panic!("Unexpected element type in result array"),
            }
        }
    }
}
