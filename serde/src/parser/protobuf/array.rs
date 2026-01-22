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
use protobuf::reflect::{FieldDescriptor, ReflectRepeatedMut, ReflectValueBox, RuntimeType};
use protobuf::MessageDyn;
use serde::de::{DeserializeSeed, Error, SeqAccess, Visitor};
use serde::Deserializer;

use crate::parser::common::LengthFieldDeserializeSeed;
use crate::parser::protobuf::complex::StructValueSeed;
use crate::parser::protobuf::string::StringValueSeed;
use someip_config::serde_config::ffi::Endianness;
use someip_serde_reflect::{
    SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
};

pub(crate) struct ArrayValueDeserializeSeed<'lt> {
    array_type_definition: &'lt SomeIpArrayTypeDefinition,
    proto_field_descriptor: &'lt FieldDescriptor,
    message: &'lt mut Box<dyn MessageDyn>,
    endianness: Endianness,
}

impl<'lt> ArrayValueDeserializeSeed<'lt> {
    pub(crate) fn new(
        array_type_definition: &'lt SomeIpArrayTypeDefinition,
        proto_field_descriptor: &'lt FieldDescriptor,
        message: &'lt mut Box<dyn MessageDyn>,
        endianness: Endianness,
    ) -> Self {
        Self { array_type_definition, proto_field_descriptor, message, endianness }
    }
}

impl<'de> DeserializeSeed<'de> for ArrayValueDeserializeSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayValueVisitor<'lt> {
            array_type_definition: &'lt SomeIpArrayTypeDefinition,
            proto_field_descriptor: &'lt FieldDescriptor,
            message: &'lt mut Box<dyn MessageDyn>,
            endianness: Endianness,
        }

        impl<'lt> ArrayValueVisitor<'lt> {
            fn new(
                array_type_definition: &'lt SomeIpArrayTypeDefinition,
                proto_field_descriptor: &'lt FieldDescriptor,
                message: &'lt mut Box<dyn MessageDyn>,
                endianness: Endianness,
            ) -> Self {
                ArrayValueVisitor {
                    array_type_definition,
                    proto_field_descriptor,
                    message,
                    endianness,
                }
            }
        }

        impl<'de> Visitor<'de> for ArrayValueVisitor<'_> {
            type Value = ();

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
                if self.proto_field_descriptor.is_singular() {
                    // this can only be a bytes field
                    let SomeIpMessageTypeDefinition::PRIMITIVE(SomeIpPrimitiveTypeDefinition::U8) =
                        *self.array_type_definition.array_type
                    else {
                        return Err(A::Error::custom("Expected a byte array in SOME/IP format"));
                    };
                    let proto_runtime_type = self.proto_field_descriptor.singular_runtime_type();
                    if proto_runtime_type != RuntimeType::VecU8 {
                        return Err(A::Error::custom("Expected a bytes field"));
                    };
                    let Some(byte_array) = seq.next_element_seed(ByteArraySeed::new(array_size))?
                    else {
                        return Err(A::Error::custom("Internal error"));
                    };
                    self.proto_field_descriptor.set_singular_field(
                        &mut **self.message,
                        ReflectValueBox::Bytes(byte_array),
                    );
                    Ok(())
                } else if self.proto_field_descriptor.is_repeated() {
                    let repeated_field =
                        self.proto_field_descriptor.mut_repeated(&mut **self.message);

                    match repeated_field.element_type() {
                        RuntimeType::VecU8 | RuntimeType::String | RuntimeType::Message(_) => {
                            seq.next_element_seed(GeneralArraySeed::new(
                                &self.array_type_definition.array_type,
                                repeated_field,
                                self.endianness,
                                array_size,
                            ))?;
                            Ok(())
                        }
                        _primitive_element_type => {
                            let SomeIpMessageTypeDefinition::PRIMITIVE(
                                ref primitive_type_definition,
                            ) = *self.array_type_definition.array_type
                            else {
                                return Err(A::Error::custom(
                                    "Expected a primitive array definition",
                                ));
                            };
                            let primitive_array_seed = PrimitiveArraySeed::new(
                                primitive_type_definition,
                                repeated_field,
                                self.endianness,
                                array_size,
                            );
                            seq.next_element_seed(primitive_array_seed)?;
                            Ok(())
                        }
                    }
                } else {
                    Err(A::Error::custom("Unknown runtime type"))
                }
            }
        }
        deserializer.deserialize_seq(ArrayValueVisitor::new(
            self.array_type_definition,
            self.proto_field_descriptor,
            self.message,
            self.endianness,
        ))
    }
}

struct ByteArraySeed {
    array_size: usize,
}

impl ByteArraySeed {
    fn new(array_size: usize) -> ByteArraySeed {
        ByteArraySeed { array_size }
    }
}

impl<'de> DeserializeSeed<'de> for ByteArraySeed {
    type Value = Vec<u8>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BytesVisitor;

        impl Visitor<'_> for BytesVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP array of bytes")
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(value)
            }
        }
        deserializer.deserialize_tuple(self.array_size, BytesVisitor)
    }
}

struct PrimitiveArraySeed<'lt> {
    primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
    repeated_field: ReflectRepeatedMut<'lt>,
    endianness: Endianness,
    array_size: usize,
}

impl<'lt> PrimitiveArraySeed<'lt> {
    fn new(
        primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
        repeated_field: ReflectRepeatedMut<'lt>,
        endianness: Endianness,
        array_size: usize,
    ) -> Self {
        Self { primitive_type_definition, repeated_field, endianness, array_size }
    }
}

impl<'de> DeserializeSeed<'de> for PrimitiveArraySeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrimitiveArrayVisitor<'lt> {
            primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
            repeated_field: ReflectRepeatedMut<'lt>,
            endianness: Endianness,
        }

        impl<'lt> PrimitiveArrayVisitor<'lt> {
            fn new(
                primitive_type_definition: &'lt SomeIpPrimitiveTypeDefinition,
                repeated_field: ReflectRepeatedMut<'lt>,
                endianness: Endianness,
            ) -> Self {
                Self { primitive_type_definition, repeated_field, endianness }
            }
        }

        impl Visitor<'_> for PrimitiveArrayVisitor<'_> {
            type Value = ();

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP array of bytes")
            }

            fn visit_byte_buf<E>(mut self, value: Vec<u8>) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let proto_element_type = self.repeated_field.element_type();
                match self.primitive_type_definition {
                    SomeIpPrimitiveTypeDefinition::BOOLEAN => {
                        if proto_element_type != RuntimeType::Bool {
                            return Err(E::custom(
                                "SOME/IP boolean array can only map to proto boolean array",
                            ));
                        }
                        for element in value {
                            self.repeated_field.push(ReflectValueBox::Bool(element != 0));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::U8 => {
                        if proto_element_type == RuntimeType::U32 {
                            for element in value {
                                self.repeated_field.push(ReflectValueBox::U32(element.into()));
                            }
                        } else if proto_element_type == RuntimeType::U64 {
                            for element in value {
                                self.repeated_field.push(ReflectValueBox::U64(element.into()));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in value {
                                self.repeated_field.push(ReflectValueBox::Enum(
                                    descriptor.clone(),
                                    element.into(),
                                ));
                            }
                        } else {
                            return Err(E::custom("SOME/IP U8 array can only be deserialized to proto u32, u64 or enum array"));
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
                        if proto_element_type == RuntimeType::U32 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::U32(element.into()));
                            }
                        } else if proto_element_type == RuntimeType::U64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::U64(element.into()));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::Enum(
                                    descriptor.clone(),
                                    element.into(),
                                ));
                            }
                        } else {
                            return Err(E::custom("SOME/IP U16 array can only be deserialized to proto u32, u64 or enum array"));
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
                        if proto_element_type == RuntimeType::U32 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::U32(element));
                            }
                        } else if proto_element_type == RuntimeType::U64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::U64(element.into()));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in values {
                                let element_value = i32::try_from(element)
                                    .map(|val_i32| {
                                        ReflectValueBox::Enum(descriptor.clone(), val_i32)
                                    })
                                    .map_err(|_| {
                                        E::custom(format!("Enum value max value is {}", i32::MAX))
                                    })?;

                                self.repeated_field.push(element_value);
                            }
                        } else {
                            return Err(E::custom("SOME/IP U32 array can only be deserialized to proto u32, u64 or enum array"));
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
                        if proto_element_type == RuntimeType::U64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::U64(element));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in values {
                                let element_value = i32::try_from(element)
                                    .map(|val_i32| {
                                        ReflectValueBox::Enum(descriptor.clone(), val_i32)
                                    })
                                    .map_err(|_| {
                                        E::custom(format!("Enum value max value is {}", i32::MAX))
                                    })?;

                                self.repeated_field.push(element_value);
                            }
                        } else {
                            return Err(E::custom("SOME/IP U64 array can only be deserialized to proto u64 or enum array"));
                        }
                    }
                    SomeIpPrimitiveTypeDefinition::I8 => {
                        if proto_element_type == RuntimeType::I32 {
                            for element in value {
                                self.repeated_field
                                    .push(ReflectValueBox::I32(element as i8 as i32));
                            }
                        } else if proto_element_type == RuntimeType::I64 {
                            for element in value {
                                self.repeated_field
                                    .push(ReflectValueBox::I64(element as i8 as i64));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in value {
                                self.repeated_field.push(ReflectValueBox::Enum(
                                    descriptor.clone(),
                                    element as i8 as i32,
                                ));
                            }
                        } else {
                            return Err(E::custom("SOME/IP I8 array can only be deserialized to proto i32, i64 or enum array"));
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
                        if proto_element_type == RuntimeType::I32 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::I32(element.into()));
                            }
                        } else if proto_element_type == RuntimeType::I64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::I64(element.into()));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::Enum(
                                    descriptor.clone(),
                                    element.into(),
                                ));
                            }
                        } else {
                            return Err(E::custom("SOME/IP I16 array can only be deserialized to proto i32, i64 or enum array"));
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
                        if proto_element_type == RuntimeType::I32 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::I32(element));
                            }
                        } else if proto_element_type == RuntimeType::I64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::I64(element.into()));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in values {
                                self.repeated_field
                                    .push(ReflectValueBox::Enum(descriptor.clone(), element));
                            }
                        } else {
                            return Err(E::custom("SOME/IP I32 array can only be deserialized to proto i32, i64 or enum array"));
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
                        if proto_element_type == RuntimeType::I64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::I64(element));
                            }
                        } else if let RuntimeType::Enum(descriptor) = proto_element_type {
                            for element in values {
                                let element_value = i32::try_from(element)
                                    .map(|val_i32| {
                                        ReflectValueBox::Enum(descriptor.clone(), val_i32)
                                    })
                                    .map_err(|_| {
                                        E::custom(format!("Enum value max value is {}", i32::MAX))
                                    })?;

                                self.repeated_field.push(element_value);
                            }
                        } else {
                            return Err(E::custom("SOME/IP I64 array can only be deserialized to proto i64 or enum array"));
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
                        if proto_element_type == RuntimeType::F32 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::F32(element));
                            }
                        } else if proto_element_type == RuntimeType::F64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::F64(element.into()));
                            }
                        } else {
                            return Err(E::custom("SOME/IP F32 array can only be deserialized to proto f32 or f64 array"));
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
                        if proto_element_type == RuntimeType::F64 {
                            for element in values {
                                self.repeated_field.push(ReflectValueBox::F64(element));
                            }
                        } else {
                            return Err(E::custom(
                                "SOME/IP F64 array can only be deserialized to proto f64 array",
                            ));
                        }
                    }
                }
                Ok(())
            }
        }

        if self.array_size == 0 {
            return Ok(());
        }
        let bytes_to_read = match self.primitive_type_definition {
            SomeIpPrimitiveTypeDefinition::BOOLEAN
            | SomeIpPrimitiveTypeDefinition::U8
            | SomeIpPrimitiveTypeDefinition::I8 => self.array_size,
            SomeIpPrimitiveTypeDefinition::U16 | SomeIpPrimitiveTypeDefinition::I16 => {
                (self.array_size / 2) * 2
            }
            SomeIpPrimitiveTypeDefinition::U32
            | SomeIpPrimitiveTypeDefinition::I32
            | SomeIpPrimitiveTypeDefinition::F32 => (self.array_size / 4) * 4,
            SomeIpPrimitiveTypeDefinition::U64
            | SomeIpPrimitiveTypeDefinition::I64
            | SomeIpPrimitiveTypeDefinition::F64 => (self.array_size / 8) * 8,
        };
        deserializer.deserialize_tuple(
            bytes_to_read,
            PrimitiveArrayVisitor::new(
                self.primitive_type_definition,
                self.repeated_field,
                self.endianness,
            ),
        )
    }
}

struct GeneralArraySeed<'lt> {
    array_element_type_definition: &'lt SomeIpMessageTypeDefinition,
    repeated_field: ReflectRepeatedMut<'lt>,
    endianness: Endianness,
    array_size: usize,
}

impl<'lt> GeneralArraySeed<'lt> {
    fn new(
        array_element_type_definition: &'lt SomeIpMessageTypeDefinition,
        repeated_field: ReflectRepeatedMut<'lt>,
        endianness: Endianness,
        array_size: usize,
    ) -> Self {
        Self { array_element_type_definition, repeated_field, endianness, array_size }
    }
}

impl<'de> DeserializeSeed<'de> for GeneralArraySeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GeneralArrayVisitor<'lt> {
            array_element_type_definition: &'lt SomeIpMessageTypeDefinition,
            repeated_field: ReflectRepeatedMut<'lt>,
            endianness: Endianness,
        }

        impl<'lt> GeneralArrayVisitor<'lt> {
            fn new(
                array_element_type_definition: &'lt SomeIpMessageTypeDefinition,
                repeated_field: ReflectRepeatedMut<'lt>,
                endianness: Endianness,
            ) -> Self {
                Self { array_element_type_definition, repeated_field, endianness }
            }
        }

        impl<'de> Visitor<'de> for GeneralArrayVisitor<'_> {
            type Value = ();

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP array")
            }

            fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let proto_runtime_type = self.repeated_field.element_type();
                match self.repeated_field.element_type() {
                    RuntimeType::VecU8 => {
                        // this is a `repeated bytes` field
                        let SomeIpMessageTypeDefinition::ARRAY(nested_array_definition) =
                            self.array_element_type_definition
                        else {
                            return Err(A::Error::custom(
                                "Expected a SOME/IP array definition for proto bytes field",
                            ));
                        };
                        loop {
                            let array_size = {
                                if let Some(size) = nested_array_definition.fixed_length {
                                    size
                                } else {
                                    let Some(size) =
                                        seq.next_element_seed(LengthFieldDeserializeSeed::new(
                                            &nested_array_definition.length_field_size,
                                        ))?
                                    else {
                                        // deserializer has no more elements
                                        break;
                                    };
                                    size
                                }
                            };
                            let Some(byte_array) =
                                seq.next_element_seed(ByteArraySeed::new(array_size))?
                            else {
                                // no more elements
                                break;
                            };
                            self.repeated_field.push(ReflectValueBox::Bytes(byte_array));
                        }
                    }
                    RuntimeType::String => {
                        let SomeIpMessageTypeDefinition::STRING(string_type_definition) =
                            self.array_element_type_definition
                        else {
                            return Err(A::Error::custom("Expected a SOME/IP string"));
                        };
                        while let Some(string_value) = seq.next_element_seed(
                            StringValueSeed::new(string_type_definition, &proto_runtime_type),
                        )? {
                            self.repeated_field.push(string_value);
                        }
                    }
                    RuntimeType::Message(msg_descriptor) => {
                        let SomeIpMessageTypeDefinition::STRUCT(struct_type_definition) =
                            self.array_element_type_definition
                        else {
                            return Err(A::Error::custom("Expected a SOME/IP struct"));
                        };

                        while let Some(struct_value) =
                            seq.next_element_seed(StructValueSeed::new(
                                struct_type_definition,
                                &msg_descriptor,
                                self.endianness,
                            ))?
                        {
                            self.repeated_field.push(ReflectValueBox::Message(struct_value));
                        }
                    }
                    _primitive_element_type => {
                        // we shouldn't be here, already handled primitive case earlier
                        return Err(A::Error::custom("Internal error"));
                    }
                }
                Ok(())
            }
        }

        deserializer.deserialize_tuple_struct(
            "",
            self.array_size,
            GeneralArrayVisitor::new(
                self.array_element_type_definition,
                self.repeated_field,
                self.endianness,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use byteorder::LittleEndian;
    use protobuf::reflect::{ReflectValueRef, RuntimeType};
    use protobuf::{EnumFull, MessageDyn, MessageFull};
    use someip_serde_reflect::{
        SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
    };

    use byteorder::BigEndian;
    use serde::de::DeserializeSeed;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize};

    use googletest::prelude::*;
    use someip_serde_test_protos::messages::ArrayMessage;

    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::protobuf::array::ArrayValueDeserializeSeed;

    #[test]
    fn test_u32_array_deserialize_succeeds() {
        let mut payload = vec![12]; // array size is 12 bytes
        let data: Vec<i32> = vec![-12, 43, -78];
        for element in &data {
            payload.extend_from_slice(&element.to_be_bytes());
        }
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(ArrayMessage::default());
        let message_descriptor = ArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert!(proto_field_descriptor.is_repeated());

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(3, repeated_values.len());
        assert_eq!(RuntimeType::I32, repeated_values.element_type());

        assert_that!(repeated_values.get(0), matches_pattern!(ReflectValueRef::I32(eq(&data[0]))));
        assert_that!(repeated_values.get(1), matches_pattern!(ReflectValueRef::I32(eq(&data[1]))));
        assert_that!(repeated_values.get(2), matches_pattern!(ReflectValueRef::I32(eq(&data[2]))));
    }

    #[test]
    fn test_deserialize_singular_bytes_dynamic_length_be() {
        use someip_serde_test_protos::messages::BytesMessage;

        let data_bytes: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let mut payload = vec![data_bytes.len() as u8]; // Length field (1 byte for array size)
        payload.extend_from_slice(&data_bytes);

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte, // Length of the byte array itself
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(BytesMessage::default());
        let message_descriptor = BytesMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("data").expect("Field 'data' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert!(proto_field_descriptor.is_singular());
        let value_ref = proto_field_descriptor.get_singular_field_or_default(&*message);
        assert_eq!(value_ref, ReflectValueRef::Bytes(data_bytes.as_slice()));
    }

    #[test]
    fn test_deserialize_repeated_u8_fixed_length_le() {
        use someip_serde_test_protos::messages::UnsignedPrimitiveArrayMessage;

        let data_u8: Vec<u8> = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let mut payload = Vec::new();
        payload.extend_from_slice(&data_u8); // For u8, bytes are direct

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(data_u8.len()),          // Fixed length in bytes
            length_field_size: LenFieldSize::ZeroBytes, // Ignored for fixed length
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(UnsignedPrimitiveArrayMessage::default());
        let message_descriptor = UnsignedPrimitiveArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_u8.len(), repeated_values.len());
        assert_eq!(RuntimeType::U32, repeated_values.element_type()); // SOME/IP u8 maps to proto u32

        for (i, item) in data_u8.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::U32(*item as u32));
        }
    }

    #[test]
    fn test_deserialize_repeated_u16_fixed_length_le() {
        use someip_serde_test_protos::messages::UnsignedPrimitiveArrayMessage; // Assuming this has a repeated u32 field

        let data_u16: Vec<u16> = vec![0x1234, 0x5678, 0x9ABC];
        let mut payload = Vec::new();
        for &element in &data_u16 {
            payload.extend_from_slice(&element.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(data_u16.len() * 2), // Fixed length in bytes
            length_field_size: LenFieldSize::ZeroBytes, // Ignored for fixed length
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U16,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(UnsignedPrimitiveArrayMessage::default());
        let message_descriptor = UnsignedPrimitiveArrayMessage::descriptor();
        // Assuming UnsignedPrimitiveArrayMessage has a field like `repeated uint32 items`
        // as SOME/IP U16 maps to proto U32 or U64.
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_u16.len(), repeated_values.len());
        assert_eq!(RuntimeType::U32, repeated_values.element_type());

        for (i, item) in data_u16.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::U32(*item as u32));
        }
    }

    #[test]
    fn test_deserialize_repeated_u32_fixed_length_be() {
        use someip_serde_test_protos::messages::UnsignedPrimitiveArrayMessage;

        let data_u32: Vec<u32> = vec![0x12345678, 0x87654321, 0xAABBCCDD];
        let mut payload = Vec::new();
        for &element in &data_u32 {
            payload.extend_from_slice(&element.to_be_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(data_u32.len() * 4), // Fixed length in bytes
            length_field_size: LenFieldSize::ZeroBytes, // Ignored for fixed length
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(UnsignedPrimitiveArrayMessage::default());
        let message_descriptor = UnsignedPrimitiveArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_u32.len(), repeated_values.len());
        assert_eq!(RuntimeType::U32, repeated_values.element_type());

        for (i, item) in data_u32.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::U32(*item));
        }
    }

    #[test]
    fn test_deserialize_repeated_u64_fixed_length_le() {
        use someip_serde_test_protos::messages::Unsigned8BytePrimitiveArrayMessage;

        let data_u64: Vec<u64> = vec![0x123456789ABCDEF0, 0xFEDCBA9876543210, 0xAABBCCDDEEFF0011];
        let mut payload = Vec::new();
        for &element in &data_u64 {
            payload.extend_from_slice(&element.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(data_u64.len() * 8), // Fixed length in bytes
            length_field_size: LenFieldSize::ZeroBytes, // Ignored for fixed length
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U64,
            )),
        };
        let mut message: Box<dyn MessageDyn> =
            Box::new(Unsigned8BytePrimitiveArrayMessage::default());
        let message_descriptor = Unsigned8BytePrimitiveArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_u64.len(), repeated_values.len());
        assert_eq!(RuntimeType::U64, repeated_values.element_type());

        for (i, item) in data_u64.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::U64(*item));
        }
    }

    #[test]
    fn test_deserialize_repeated_i8_dynamic_length_be() {
        use someip_serde_test_protos::messages::ArrayMessage;

        let data_i8: Vec<i8> = vec![-10, 20, -30, 0, 127, -128];
        let mut payload = vec![data_i8.len() as u8]; // Length field (1 byte for array size in elements)
        for &element in &data_i8 {
            payload.push(element as u8); // i8 to u8 for byte stream
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I8,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(ArrayMessage::default());
        let message_descriptor = ArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field 'field' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_i8.len(), repeated_values.len());
        assert_eq!(RuntimeType::I32, repeated_values.element_type()); // SOME/IP i8 maps to proto i32

        for (i, item) in data_i8.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::I32(*item as i32));
        }
    }

    #[test]
    fn test_deserialize_repeated_i16_dynamic_length_le() {
        use someip_serde_test_protos::messages::ArrayMessage;

        let data_i16: Vec<i16> = vec![-1000, 2000, -3000, 0x7FFF, -0x8000];
        // Length is number of bytes: data_i16.len() * 2
        let mut payload = vec![(data_i16.len() * 2) as u8]; // Length field (1 byte for array size in bytes)
        for &element in &data_i16 {
            payload.extend_from_slice(&element.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I16,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(ArrayMessage::default());
        let message_descriptor = ArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field 'field' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_i16.len(), repeated_values.len());
        assert_eq!(RuntimeType::I32, repeated_values.element_type()); // SOME/IP i16 maps to proto i32

        for (i, item) in data_i16.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::I32(*item as i32));
        }
    }

    #[test]
    fn test_deserialize_repeated_i32_dynamic_length_be() {
        use someip_serde_test_protos::messages::ArrayMessage;

        let data_i32: Vec<i32> = vec![-100000, 200000, -300000];
        // Length is number of bytes: data_i32.len() * 4
        let mut payload = vec![(data_i32.len() * 4) as u8]; // Length field (1 byte for array size in bytes)
        for &element in &data_i32 {
            payload.extend_from_slice(&element.to_be_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(ArrayMessage::default());
        let message_descriptor = ArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field 'field' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_i32.len(), repeated_values.len());
        assert_eq!(RuntimeType::I32, repeated_values.element_type());

        for (i, item) in data_i32.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::I32(*item));
        }
    }

    // Test for repeated i64 array (dynamic length, little endian)
    #[test]
    fn test_deserialize_repeated_i64_dynamic_length_le() {
        use someip_serde_test_protos::messages::Signed8BytePrimitiveArrayMessage;

        let data_i64: Vec<i64> = vec![-123456789012345, 987654321098765, -0x7FFFFFFFFFFFFFFF];
        // Length is number of bytes: data_i64.len() * 8
        // Using 2 bytes for length field to accommodate larger array byte sizes
        let array_byte_len = (data_i64.len() * 8) as u16;
        let mut payload = array_byte_len.to_le_bytes().to_vec(); // Length field (2 bytes for array size in bytes)

        for &element in &data_i64 {
            payload.extend_from_slice(&element.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::TwoBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I64,
            )),
        };
        let mut message: Box<dyn MessageDyn> =
            Box::new(Signed8BytePrimitiveArrayMessage::default());
        let message_descriptor = Signed8BytePrimitiveArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_i64.len(), repeated_values.len());
        assert_eq!(RuntimeType::I64, repeated_values.element_type());

        for (i, item) in data_i64.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::I64(*item));
        }
    }

    #[test]
    fn test_deserialize_repeated_f32_dynamic_length_be() {
        use someip_serde_test_protos::messages::PrimitiveFloatArrayMessage;

        let data_f32: Vec<f32> = vec![-10.5, 20.0, -30.75, 0.0, 127.125];
        // Length is number of bytes: data_f32.len() * 4
        let mut payload = vec![(data_f32.len() * 4) as u8]; // Length field (1 byte for array size in bytes)
        for &element in &data_f32 {
            payload.extend_from_slice(&element.to_be_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::F32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(PrimitiveFloatArrayMessage::default());
        let message_descriptor = PrimitiveFloatArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_f32.len(), repeated_values.len());
        assert_eq!(RuntimeType::F32, repeated_values.element_type());

        for (i, item) in data_f32.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::F32(*item));
        }
    }

    #[test]
    fn test_deserialize_repeated_f64_dynamic_length_le() {
        use someip_serde_test_protos::messages::PrimitiveDoubleArrayMessage;

        let data_f64: Vec<f64> = vec![-123.456, 789.012, -0.00000000005];
        // Length is number of bytes: data_f64.len() * 8
        let array_byte_len = (data_f64.len() * 8) as u16;
        let mut payload = array_byte_len.to_le_bytes().to_vec(); // Length field (2 bytes)

        for &element in &data_f64 {
            payload.extend_from_slice(&element.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::TwoBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::F64,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(PrimitiveDoubleArrayMessage::default());
        let message_descriptor = PrimitiveDoubleArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_f64.len(), repeated_values.len());
        assert_eq!(RuntimeType::F64, repeated_values.element_type());

        for (i, item) in data_f64.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::F64(*item));
        }
    }

    #[test]
    fn test_deserialize_repeated_bool_dynamic_length_be() {
        use someip_serde_test_protos::messages::PrimitiveBoolArrayMessage;

        let data_bool: Vec<bool> = vec![true, false, true, true, false];
        // Length is number of bytes (elements): data_bool.len()
        let mut payload = vec![data_bool.len() as u8]; // Length field (1 byte for array size in elements/bytes)
        for &element in &data_bool {
            payload.push(if element { 0x01 } else { 0x00 });
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::BOOLEAN,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(PrimitiveBoolArrayMessage::default());
        let message_descriptor = PrimitiveBoolArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian, // Endianness for length field
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(data_bool.len(), repeated_values.len());
        assert_eq!(RuntimeType::Bool, repeated_values.element_type());

        for (i, item) in data_bool.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::Bool(*item));
        }
    }

    #[test]
    fn test_deserialize_repeated_enum_from_u8_dynamic_length_be() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> =
            vec![TestEnum::FIRST_VALUE, TestEnum::SECOND_VALUE, TestEnum::THIRD_VALUE];
        let int_data: Vec<u8> = enum_data.iter().map(|e| *e as u8).collect();

        // Payload: length (1 byte, total bytes of elements) + u8 elements
        let mut payload = vec![int_data.len() as u8]; // For u8, num_elements == num_bytes
        payload.extend_from_slice(&int_data);

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    #[test]
    fn test_deserialize_repeated_enum_from_i16_fixed_length_le() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> = vec![TestEnum::THIRD_VALUE, TestEnum::FIRST_VALUE];
        let int_data: Vec<i16> = enum_data.iter().map(|e| *e as i16).collect();

        let mut payload = Vec::new();
        for &val in &int_data {
            payload.extend_from_slice(&val.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(int_data.len() * 2), // num_elements * sizeof(i16)
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I16,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    #[test]
    fn test_deserialize_repeated_enum_from_u32_dynamic_length_be() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> = vec![TestEnum::SECOND_VALUE];
        let int_data: Vec<u32> = enum_data.iter().map(|e| *e as u32).collect();

        let total_bytes = (int_data.len() * 4) as u16; // u16 for length field
        let mut payload = total_bytes.to_be_bytes().to_vec(); // 2-byte length field
        for &val in &int_data {
            payload.extend_from_slice(&val.to_be_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::TwoBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    #[test]
    fn test_deserialize_repeated_enum_from_i64_fixed_length_le() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> =
            vec![TestEnum::FIRST_VALUE, TestEnum::SECOND_VALUE, TestEnum::THIRD_VALUE];
        let int_data: Vec<i64> = enum_data.iter().map(|e| *e as i64).collect();

        let mut payload = Vec::new();
        for &val in &int_data {
            payload.extend_from_slice(&val.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(int_data.len() * 8), // num_elements * sizeof(i64)
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I64,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    #[test]
    fn test_deserialize_empty_repeated_enum_dynamic_length() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        // Payload: length (1 byte, value 0 for empty array)
        let payload = vec![0u8];

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8, // Underlying type doesn't matter much for empty
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(0, repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );
    }

    #[test]
    fn test_deserialize_repeated_enum_from_i8_dynamic_length_le() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> = vec![TestEnum::FIRST_VALUE, TestEnum::SECOND_VALUE];
        let int_data: Vec<i8> = enum_data.iter().map(|e| *e as i8).collect();

        // Payload: length (1 byte, total bytes of elements) + i8 elements (as u8)
        let mut payload = vec![int_data.len() as u8];
        for &val in &int_data {
            payload.push(val as u8);
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I8,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    #[test]
    fn test_deserialize_repeated_enum_from_u16_fixed_length_be() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> =
            vec![TestEnum::THIRD_VALUE, TestEnum::FIRST_VALUE, TestEnum::SECOND_VALUE];
        let int_data: Vec<u16> = enum_data.iter().map(|e| *e as u16).collect();

        let mut payload = Vec::new();
        for &val in &int_data {
            payload.extend_from_slice(&val.to_be_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(int_data.len() * 2), // num_elements * sizeof(u16)
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U16,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    #[test]
    fn test_deserialize_repeated_enum_from_i32_dynamic_length_le() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> = vec![TestEnum::FIRST_VALUE, TestEnum::THIRD_VALUE];
        let int_data: Vec<i32> = enum_data.iter().map(|e| *e as i32).collect();

        let total_bytes = (int_data.len() * 4) as u32; // u32 for length field
        let mut payload = total_bytes.to_le_bytes().to_vec(); // 4-byte length field
        for &val in &int_data {
            payload.extend_from_slice(&val.to_le_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::FourBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    #[test]
    fn test_deserialize_repeated_enum_from_u64_fixed_length_be() {
        use someip_serde_test_protos::messages::{EnumArrayMessage, TestEnum};

        let enum_data: Vec<TestEnum> = vec![TestEnum::SECOND_VALUE];
        let int_data: Vec<u64> = enum_data.iter().map(|e| *e as u64).collect();

        let mut payload = Vec::new();
        for &val in &int_data {
            payload.extend_from_slice(&val.to_be_bytes());
        }

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: Some(int_data.len() * 8),
            length_field_size: LenFieldSize::ZeroBytes,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U64,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(EnumArrayMessage::default());
        let message_descriptor = EnumArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("items").expect("Field 'items' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(enum_data.len(), repeated_values.len());
        let expected_enum_descriptor = TestEnum::enum_descriptor();
        assert_eq!(
            RuntimeType::Enum(expected_enum_descriptor.clone()),
            repeated_values.element_type()
        );

        for (i, expected_enum) in enum_data.iter().enumerate() {
            assert_eq!(
                repeated_values.get(i),
                ReflectValueRef::Enum(expected_enum_descriptor.clone(), *expected_enum as i32)
            );
        }
    }

    // Test for repeated string array (dynamic length)
    #[test]
    fn test_deserialize_repeated_string_array_dynamic_length_be() {
        use someip_config::serde_config::ffi::StringType;
        use someip_serde_reflect::SomeIpStringTypeDefinition;
        use someip_serde_test_protos::messages::RepeatedStringMessage;

        let strings_data: Vec<String> =
            vec!["hello".to_string(), "world".to_string(), "".to_string()];
        let mut string_elements_payload = Vec::new();
        let string_type_def = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte, // Length of each string
            bom: StringType::UTF_8,                   // Assuming UTF-8 for simplicity
            fixed_length: None,
        };

        // Manually serialize each string element as it would appear in SOME/IP
        for s in &strings_data {
            let mut single_string_payload = Vec::new();
            // BOM + content + null terminator
            let content_bytes = s.as_bytes();
            let bom = &[0xEF, 0xBB, 0xBF]; // UTF-8 BOM
            let string_data_len = bom.len() + content_bytes.len() + 1;

            single_string_payload.push(string_data_len as u8); // Length of string data
            single_string_payload.extend_from_slice(bom);
            single_string_payload.extend_from_slice(content_bytes);
            single_string_payload.push(0x00); // Null terminator
            string_elements_payload.extend_from_slice(&single_string_payload);
        }

        let mut payload = vec![string_elements_payload.len() as u8]; // Outer array length
        payload.extend_from_slice(&string_elements_payload);

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte, // Length of the outer array
            array_type: Box::new(SomeIpMessageTypeDefinition::STRING(string_type_def)),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(RepeatedStringMessage::default());
        let message_descriptor = RepeatedStringMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field 'field' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(strings_data.len(), repeated_values.len());
        assert_eq!(RuntimeType::String, repeated_values.element_type());

        for (i, item) in strings_data.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::String(item.as_str()));
        }
    }

    #[test]
    fn test_deserialize_repeated_string_array_dynamic_length_le() {
        use someip_config::serde_config::ffi::StringType;
        use someip_serde_reflect::SomeIpStringTypeDefinition;
        use someip_serde_test_protos::messages::RepeatedStringMessage;

        let strings_data: Vec<String> =
            vec!["first".to_string(), "second_item".to_string(), "3rd".to_string()];
        let mut string_elements_payload = Vec::new();
        let string_type_def = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte, // Length of each string
            bom: StringType::UTF_8,                   // Assuming UTF-8 for simplicity
            fixed_length: None,
        };

        // Manually serialize each string element as it would appear in SOME/IP
        for s in &strings_data {
            let mut single_string_payload = Vec::new();
            // BOM + content + null terminator
            let content_bytes = s.as_bytes();
            let bom = &[0xEF, 0xBB, 0xBF]; // UTF-8 BOM
            let string_data_len = bom.len() + content_bytes.len() + 1;

            single_string_payload.push(string_data_len as u8); // Length of string data
            single_string_payload.extend_from_slice(bom);
            single_string_payload.extend_from_slice(content_bytes);
            single_string_payload.push(0x00); // Null terminator
            string_elements_payload.extend_from_slice(&single_string_payload);
        }

        let mut payload = vec![string_elements_payload.len() as u8]; // Outer array length (1 byte)
        payload.extend_from_slice(&string_elements_payload);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte, // Length of the outer array
            array_type: Box::new(SomeIpMessageTypeDefinition::STRING(string_type_def)),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(RepeatedStringMessage::default());
        let message_descriptor = RepeatedStringMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field 'field' must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::LittleEndian,
        );

        array_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let repeated_values = proto_field_descriptor.get_repeated(&*message);
        assert_eq!(strings_data.len(), repeated_values.len());
        assert_eq!(RuntimeType::String, repeated_values.element_type());

        for (i, item) in strings_data.iter().enumerate() {
            assert_eq!(repeated_values.get(i), ReflectValueRef::String(item.as_str()));
        }
    }

    // Test for error when array size is expected but not found
    #[test]
    fn test_deserialize_error_missing_array_size() {
        let payload = vec![]; // Empty payload, so length field cannot be read

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None, // Dynamic length
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(ArrayMessage::default());
        let message_descriptor = ArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        let result = array_value_seed.deserialize(&mut deserializer);
        assert!(result.is_err());
    }

    // Test for primitive array where byte stream is too short for the declared length
    #[test]
    fn test_deserialize_primitive_array_insufficient_data() {
        let array_declared_bytes = 12; // Expect 3 i32s
        let mut payload = vec![array_declared_bytes as u8];
        payload.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]); // Only one i32
        payload.extend_from_slice(&[0x05, 0x06]); // Incomplete second i32

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload);
        let array_type_definition = SomeIpArrayTypeDefinition {
            fixed_length: None,
            length_field_size: LenFieldSize::OneByte,
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I32,
            )),
        };
        let mut message: Box<dyn MessageDyn> = Box::new(ArrayMessage::default());
        let message_descriptor = ArrayMessage::descriptor();
        let proto_field_descriptor =
            message_descriptor.field_by_name("field").expect("Field must be available");

        let array_value_seed = ArrayValueDeserializeSeed::new(
            &array_type_definition,
            &proto_field_descriptor,
            &mut message,
            Endianness::BigEndian,
        );

        // This will likely error out inside deserialize_tuple or when trying to read past buffer.
        // The exact error might depend on the deserializer's internal error handling for short reads.
        let result = array_value_seed.deserialize(&mut deserializer);
        assert!(result.is_err());
    }
}
