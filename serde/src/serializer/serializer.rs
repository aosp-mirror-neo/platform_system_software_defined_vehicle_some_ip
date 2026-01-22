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

use byteorder::{ByteOrder, WriteBytesExt};

use serde::ser::Impossible;
use serde::ser::SerializeSeq;
use serde::Serializer as SerdeSerializer;

use someip_config::serde_config::ffi::{Endianness, SomeIpConfig};

use crate::serializer::error::SomeIpSerializeError;

/// SOME/IP payload serializer
pub struct SomeIpSerdeSerializer<B: ByteOrder> {
    output: Vec<u8>,
    index: usize,
    config: SomeIpConfig,
    _pd: std::marker::PhantomData<B>,
}

impl<B: ByteOrder> SomeIpSerdeSerializer<B> {
    pub(crate) fn new(config: SomeIpConfig) -> Self {
        SomeIpSerdeSerializer {
            output: Vec::new(),
            index: 0,
            config,
            _pd: std::marker::PhantomData,
        }
    }

    pub(crate) fn get_serialized_bytes(self) -> Vec<u8> {
        self.output
    }
}

impl<'serializer_lt, B: ByteOrder> SerdeSerializer
    for &'serializer_lt mut SomeIpSerdeSerializer<B>
{
    type Ok = ();

    type Error = SomeIpSerializeError;

    type SerializeSeq = SomeIpSeqSerializer<'serializer_lt, B>;

    type SerializeStruct = Impossible<(), SomeIpSerializeError>;

    type SerializeTuple = Impossible<(), SomeIpSerializeError>;

    type SerializeTupleStruct = Impossible<(), SomeIpSerializeError>;

    type SerializeTupleVariant = Impossible<(), SomeIpSerializeError>;

    type SerializeMap = Impossible<(), SomeIpSerializeError>;

    type SerializeStructVariant = Impossible<(), SomeIpSerializeError>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        if value {
            self.output.write_u8(0x01).map_err(|_| SomeIpSerializeError::Primitive)?;
        } else {
            self.output.write_u8(0x00).map_err(|_| SomeIpSerializeError::Primitive)?;
        }
        self.index += 1;
        Ok(())
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.output.write_i8(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 1;
        Ok(())
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.output.write_i16::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 2;
        Ok(())
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.output.write_i32::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 4;
        Ok(())
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        self.output.write_i64::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 8;
        Ok(())
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.output.write_u8(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 1;
        Ok(())
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.output.write_u16::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 2;
        Ok(())
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.output.write_u32::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 4;
        Ok(())
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        self.output.write_u64::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 8;
        Ok(())
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        self.output.write_f32::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 4;
        Ok(())
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        self.output.write_f64::<B>(value).map_err(|_| SomeIpSerializeError::Primitive)?;
        self.index += 8;
        Ok(())
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        unreachable!("serialize_char should not be used for SOME/IP serialization")
    }

    fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
        unreachable!("serialize_str should not be used for SOME/IP serialization")
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.output.extend_from_slice(value);
        self.index += value.len();
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        unreachable!("serialize_none should not be used for SOME/IP serialization")
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        unreachable!("serialize_some should not be used for SOME/IP serialization")
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        unreachable!("serialize_unit should not be used for SOME/IP serialization")
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        unreachable!("serialize_unit_struct should not be used for SOME/IP serialization")
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        unreachable!("serialize_unit_variant should not be used for SOME/IP serialization")
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        unreachable!("serialize_newtype_struct should not be used for SOME/IP serialization")
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        unreachable!("serialize_newtype_variant should not be used for SOME/IP serialization")
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        match len {
            Some(len_field_size) => {
                let starting_index = self.output.len();
                match len_field_size {
                    1 => self.serialize_u8(0)?,
                    2 => self.serialize_u16(0)?,
                    4 => self.serialize_u32(0)?,
                    _ => {
                        return Err(SomeIpSerializeError::Failure(
                            format!("Internal error, serialize_seq: unknown length field size {len_field_size} for dynamic size data")
                        ));
                    }
                }
                Ok(SomeIpSeqSerializer {
                    serializer: self,
                    dynamic_size_seq_data: Some(DynamicSizeSeqData {
                        starting_index,
                        len_field_size,
                    }),
                })
            }
            None => Ok(SomeIpSeqSerializer { serializer: self, dynamic_size_seq_data: None }),
        }
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        unreachable!("serialize_tuple should not be used for SOME/IP serialization")
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        unreachable!("serialize_tuple_struct should not be used for SOME/IP serialization")
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        unreachable!("serialize_tuple_variant should not be used for SOME/IP serialization")
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        unreachable!("serialize_map should not be used for SOME/IP serialization")
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        unreachable!("serialize_struct should not be used for SOME/IP serialization")
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        unreachable!("serialize_struct_variant should not be used for SOME/IP serialization")
    }
}

/// SOME/IP sequential serializer
pub struct SomeIpSeqSerializer<'serializer_lt, B: ByteOrder> {
    serializer: &'serializer_lt mut SomeIpSerdeSerializer<B>,
    dynamic_size_seq_data: Option<DynamicSizeSeqData>,
}

impl<B: ByteOrder> SerializeSeq for SomeIpSeqSerializer<'_, B> {
    type Ok = ();

    type Error = SomeIpSerializeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(dynamic_size_seq_data) = self.dynamic_size_seq_data {
            let dynamic_seq_byte_length = self.serializer.output.len()
                - dynamic_size_seq_data.starting_index
                - dynamic_size_seq_data.len_field_size;
            match dynamic_size_seq_data.len_field_size {
                1 => {
                    let length = u8::try_from(dynamic_seq_byte_length).map_err(|_| {
                        SomeIpSerializeError::Failure(
                            "Internal error, serialized seq length is more than u8::max"
                                .to_string(),
                        )
                    })?;
                    if dynamic_size_seq_data.starting_index < self.serializer.output.len() {
                        self.serializer.output[dynamic_size_seq_data.starting_index] = length;
                    } else {
                        return Err(SomeIpSerializeError::Failure(
                            "Internal error, invalid SOME/IP seq value length index".to_string(),
                        ));
                    }
                }
                2 => {
                    let length = u16::try_from(dynamic_seq_byte_length).map_err(|_| {
                        SomeIpSerializeError::Failure(
                            "Internal error, serialized seq length is more than u16::max"
                                .to_string(),
                        )
                    })?;
                    if dynamic_size_seq_data.starting_index < self.serializer.output.len() - 1 {
                        let serialized_length = match self.serializer.config.endianness {
                            Endianness::BigEndian => length.to_be_bytes(),
                            Endianness::LittleEndian => length.to_le_bytes(),
                            _ => {
                                return Err(SomeIpSerializeError::Failure(
                                    "Internal error, unknown endianness".to_string(),
                                ));
                            }
                        };
                        let mut i = dynamic_size_seq_data.starting_index;
                        for el in serialized_length {
                            self.serializer.output[i] = el;
                            i += 1;
                        }
                    } else {
                        return Err(SomeIpSerializeError::Failure(
                            "Internal error, invalid SOME/IP seq value length index".to_string(),
                        ));
                    }
                }
                4 => {
                    let length = u32::try_from(dynamic_seq_byte_length).map_err(|_| {
                        SomeIpSerializeError::Failure(
                            "Internal error, serialized seq length is more than u32::max"
                                .to_string(),
                        )
                    })?;
                    if dynamic_size_seq_data.starting_index < self.serializer.output.len() - 3 {
                        let serialized_length = match self.serializer.config.endianness {
                            Endianness::BigEndian => length.to_be_bytes(),
                            Endianness::LittleEndian => length.to_le_bytes(),
                            _ => {
                                return Err(SomeIpSerializeError::Failure(
                                    "Internal error, unknown endianness".to_string(),
                                ));
                            }
                        };
                        let mut i = dynamic_size_seq_data.starting_index;
                        for el in serialized_length {
                            self.serializer.output[i] = el;
                            i += 1;
                        }
                    } else {
                        return Err(SomeIpSerializeError::Failure(
                            "Internal error, invalid SOME/IP seq value length index".to_string(),
                        ));
                    }
                }
                _ => {
                    return Err(SomeIpSerializeError::Failure(
                        "Internal error, end: unknown length field size for dynamic size data"
                            .to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

struct DynamicSizeSeqData {
    starting_index: usize,
    len_field_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::{BigEndian, LittleEndian};
    use serde::Serialize;
    use someip_config::serde_config::ffi::{LenFieldSize, StringType};

    fn default_config(endianness: Endianness) -> SomeIpConfig {
        SomeIpConfig {
            endianness,
            string_type: StringType::UTF_8,
            len_field_size: LenFieldSize::ZeroBytes,
        }
    }

    #[test]
    fn test_serialize_bool() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        true.serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x01]);

        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        false.serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x00]);
    }

    #[test]
    fn test_serialize_i8() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (42i8).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![42]);

        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (-42i8).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![(-42i8) as u8]);
    }

    #[test]
    fn test_serialize_u8() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (42u8).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![42]);
    }

    #[test]
    fn test_serialize_i16_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (0x1234i16).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x12, 0x34]);
    }

    #[test]
    fn test_serialize_i16_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (0x1234i16).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x34, 0x12]);
    }

    #[test]
    fn test_serialize_u16_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (0x1234u16).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x12, 0x34]);
    }

    #[test]
    fn test_serialize_u16_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (0x1234u16).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x34, 0x12]);
    }

    #[test]
    fn test_serialize_i32_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (0x12345678i32).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_serialize_i32_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (0x12345678i32).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_serialize_u32_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (0x12345678u32).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_serialize_u32_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (0x12345678u32).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_serialize_i64_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (0x123456789abcdef0i64).serialize(&mut serializer).unwrap();
        assert_eq!(
            serializer.get_serialized_bytes(),
            vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0]
        );
    }

    #[test]
    fn test_serialize_i64_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (0x123456789abcdef0i64).serialize(&mut serializer).unwrap();
        assert_eq!(
            serializer.get_serialized_bytes(),
            vec![0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12]
        );
    }

    #[test]
    fn test_serialize_u64_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (0x123456789abcdef0u64).serialize(&mut serializer).unwrap();
        assert_eq!(
            serializer.get_serialized_bytes(),
            vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0]
        );
    }

    #[test]
    fn test_serialize_u64_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (0x123456789abcdef0u64).serialize(&mut serializer).unwrap();
        assert_eq!(
            serializer.get_serialized_bytes(),
            vec![0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12]
        );
    }

    #[test]
    fn test_serialize_f32_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (1.23f32).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), 1.23f32.to_be_bytes());
    }

    #[test]
    fn test_serialize_f32_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (1.23f32).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), 1.23f32.to_le_bytes());
    }

    #[test]
    fn test_serialize_f64_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        (1.23f64).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), 1.23f64.to_be_bytes());
    }

    #[test]
    fn test_serialize_f64_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        (1.23f64).serialize(&mut serializer).unwrap();
        assert_eq!(serializer.get_serialized_bytes(), 1.23f64.to_le_bytes());
    }

    #[test]
    fn test_serialize_seq_dynamic_size_u8_len_big_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        let data: Vec<u8> = vec![10, 20, 30];
        // Simulate how a struct with a dynamic array might call serialize_seq
        let mut seq_serializer = serializer.serialize_seq(Some(1)).unwrap();
        for item in &data {
            seq_serializer.serialize_element(item).unwrap();
        }
        seq_serializer.end().unwrap();
        assert_eq!(serializer.get_serialized_bytes(), vec![3u8, 10, 20, 30]);
    }

    #[test]
    fn test_serialize_seq_dynamic_size_u16_len_little_endian() {
        let mut serializer =
            SomeIpSerdeSerializer::<LittleEndian>::new(default_config(Endianness::LittleEndian));
        let data: Vec<u16> = vec![0x1122, 0x3344];
        let mut seq_serializer = serializer.serialize_seq(Some(2)).unwrap(); // u16 length field
        for item in &data {
            seq_serializer.serialize_element(item).unwrap();
        }
        seq_serializer.end().unwrap();
        // Length is 4 bytes (2 elements * 2 bytes/element)
        assert_eq!(serializer.get_serialized_bytes(), vec![0x04, 0x00, 0x22, 0x11, 0x44, 0x33]);
    }

    #[test]
    #[should_panic(expected = "serialize_char should not be used for SOME/IP serialization")]
    fn test_serialize_char_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        'a'.serialize(&mut serializer).unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_str should not be used for SOME/IP serialization")]
    fn test_serialize_str_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        "test".serialize(&mut serializer).unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_none should not be used for SOME/IP serialization")]
    fn test_serialize_none_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_none().unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_some should not be used for SOME/IP serialization")]
    fn test_serialize_some_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_some(&42).unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_unit should not be used for SOME/IP serialization")]
    fn test_serialize_unit_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_unit().unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_unit_struct should not be used for SOME/IP serialization")]
    fn test_serialize_unit_struct_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_unit_struct("TestStruct").unwrap();
    }

    #[test]
    #[should_panic(
        expected = "serialize_unit_variant should not be used for SOME/IP serialization"
    )]
    fn test_serialize_unit_variant_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_unit_variant("TestEnum", 0, "TestVariant").unwrap();
    }

    #[test]
    #[should_panic(
        expected = "serialize_newtype_struct should not be used for SOME/IP serialization"
    )]
    fn test_serialize_newtype_struct_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_newtype_struct("TestNewtype", &42).unwrap();
    }

    #[test]
    #[should_panic(
        expected = "serialize_newtype_variant should not be used for SOME/IP serialization"
    )]
    fn test_serialize_newtype_variant_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_newtype_variant("TestEnum", 0, "TestVariant", &42).unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_tuple should not be used for SOME/IP serialization")]
    fn test_serialize_tuple_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_tuple(2).unwrap();
    }

    #[test]
    #[should_panic(
        expected = "serialize_tuple_struct should not be used for SOME/IP serialization"
    )]
    fn test_serialize_tuple_struct_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_tuple_struct("TestTupleStruct", 2).unwrap();
    }

    #[test]
    #[should_panic(
        expected = "serialize_tuple_variant should not be used for SOME/IP serialization"
    )]
    fn test_serialize_tuple_variant_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_tuple_variant("TestEnum", 0, "TestVariant", 2).unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_map should not be used for SOME/IP serialization")]
    fn test_serialize_map_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_map(Some(2)).unwrap();
    }

    #[test]
    #[should_panic(expected = "serialize_struct should not be used for SOME/IP serialization")]
    fn test_serialize_struct_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_struct("TestStruct", 2).unwrap();
    }

    #[test]
    #[should_panic(
        expected = "serialize_struct_variant should not be used for SOME/IP serialization"
    )]
    fn test_serialize_struct_variant_panics() {
        let mut serializer =
            SomeIpSerdeSerializer::<BigEndian>::new(default_config(Endianness::BigEndian));
        serializer.serialize_struct_variant("TestEnum", 0, "TestVariant", 2).unwrap();
    }
}
