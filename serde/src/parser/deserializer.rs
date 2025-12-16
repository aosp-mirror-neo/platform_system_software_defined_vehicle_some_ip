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
//! Contains method for parsing SOME/IP wire format into structured data

use std::cmp::Ordering;

use byteorder::ByteOrder;
use serde::de::{DeserializeSeed, SeqAccess};
use someip_dll_api::{ISomeIpDeserializer, SomeIpDeserializer};

use crate::parser::error::SomeIpParseError;

/// SOME/IP payload deserializer
pub(crate) struct SomeIpSerdeDeserializer<'buffer_lt, B: ByteOrder> {
    deserializer: SomeIpDeserializer<'buffer_lt, B>,
    input_length: usize,
}

impl<'buffer_lt, B: ByteOrder> SomeIpSerdeDeserializer<'buffer_lt, B> {
    pub fn new(input: &'buffer_lt [u8]) -> Self {
        let input_length = input.len();
        SomeIpSerdeDeserializer { deserializer: SomeIpDeserializer::new(input), input_length }
    }

    /// Returns the number of bytes read so far by the inner deserializer.
    pub(crate) fn num_read_bytes(&self) -> usize {
        self.deserializer.num_read_bytes()
    }
}

/// SOME/IP tuple deserializer. This deserializer deserializes elements in sequence until `size`
/// bytes have been read from the input
pub(crate) struct SomeIpTupleDeserializer<'des_lt, 'buffer_lt, B: ByteOrder> {
    some_ip_deserializer: &'des_lt mut SomeIpSerdeDeserializer<'buffer_lt, B>,
    bytes_to_read: usize,
    initial_bytes_read: usize,
}

/// Implementation of 'SeqAccess' trait for 'SomeIpTupleDeserializer'. This implementation is used by a serde 'Visitor' to deserialize elements in sequence
/// See https://docs.rs/serde/latest/serde/de/trait.SeqAccess.html for more details.
impl<'buffer_lt, B: ByteOrder> SeqAccess<'buffer_lt>
    for SomeIpTupleDeserializer<'_, 'buffer_lt, B>
{
    type Error = SomeIpParseError;
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'buffer_lt>,
    {
        let bytes_read =
            self.some_ip_deserializer.deserializer.num_read_bytes() - self.initial_bytes_read;

        match bytes_read.cmp(&self.bytes_to_read) {
            Ordering::Less => seed.deserialize(&mut *self.some_ip_deserializer).map(Some),
            Ordering::Equal => Ok(None),
            Ordering::Greater => Err(SomeIpParseError::OutOfRange),
        }
    }
}

/// SOME/IP seq deserializer. This deserializer relies on the visitor calling it to know when
/// to stop calling `next_element_seed`
pub(crate) struct SomeIpSeqDeserializer<'des_lt, 'buffer_lt, B: ByteOrder> {
    some_ip_deserializer: &'des_lt mut SomeIpSerdeDeserializer<'buffer_lt, B>,
}

/// Implementation of 'SeqAccess' trait for 'SomeIpSeqDeserializer'. This implementation is used by a serde 'Visitor' to deserialize array elements
/// See https://docs.rs/serde/latest/serde/de/trait.SeqAccess.html for more details.
impl<'buffer_lt, B: ByteOrder> SeqAccess<'buffer_lt> for SomeIpSeqDeserializer<'_, 'buffer_lt, B> {
    type Error = SomeIpParseError;
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'buffer_lt>,
    {
        // Deserialize element. Visitor will know when to stop
        seed.deserialize(&mut *self.some_ip_deserializer).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        self.some_ip_deserializer
            .input_length
            .checked_sub(self.some_ip_deserializer.num_read_bytes())
    }
}

impl<'buffer_lt, B: ByteOrder> serde::Deserializer<'buffer_lt>
    for &mut SomeIpSerdeDeserializer<'buffer_lt, B>
{
    type Error = SomeIpParseError;

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value =
            self.deserializer.deserialize_bool().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_bool(value)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_i8().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_i8(value)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_i16().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_i16(value)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_i32().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_i32(value)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_i64().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_i64(value)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_u8().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_u8(value)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_u16().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_u16(value)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_u32().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_u32(value)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_u64().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_u64(value)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_f32().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_f32(value)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_f64().map_err(|_| SomeIpParseError::Primitive)?;
        visitor.visit_f64(value)
    }

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_any should not be used for SOME/IP deserialization")
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_char should not be used for SOME/IP deserialization")
    }

    fn deserialize_str<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_str should not be used for SOME/IP deserialization")
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let value = self.deserializer.deserialize_string().map_err(|_| SomeIpParseError::String)?;
        visitor.visit_string(value)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_bytes should not be used for SOME/IP deserialization")
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_byte_buf should not be used for SOME/IP deserialization")
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_option should not be used for SOME/IP deserialization")
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_unit should not be used for SOME/IP deserialization")
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_unit_struct should not be used for SOME/IP deserialization")
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_newtype_struct should not be used for SOME/IP deserialization")
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let sequential_deserializer = SomeIpSeqDeserializer { some_ip_deserializer: self };
        visitor.visit_seq(sequential_deserializer)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let data = self
            .deserializer
            .read_bytes(len)
            .map_err(|e| SomeIpParseError::Failure(format!("Failed to read {len} bytes: {e}")))?;
        visitor.visit_byte_buf(data)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _: &'static str,
        bytes_to_read: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        let initial_bytes_read = self.deserializer.num_read_bytes();
        let tuple_deserializer = SomeIpTupleDeserializer {
            some_ip_deserializer: self,
            bytes_to_read,
            initial_bytes_read,
        };
        visitor.visit_seq(tuple_deserializer)
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_map should not be used for SOME/IP deserialization")
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_struct should not be used for SOME/IP deserialization")
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_enum should not be used for SOME/IP deserialization")
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_identifier should not be used for SOME/IP deserialization")
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'buffer_lt>,
    {
        unreachable!("deserialize_ignored_any should not be used for SOME/IP deserialization")
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use byteorder::BigEndian;

    use serde::de::{SeqAccess, Visitor};
    use serde::Deserializer;

    use crate::parser::deserializer::SomeIpSerdeDeserializer;

    const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];

    #[derive(Default)]
    struct TestState {
        visited_bools: Vec<bool>,
        visited_u8s: Vec<u8>,
        visited_u16s: Vec<u16>,
        visited_u32s: Vec<u32>,
        visited_u64s: Vec<u64>,

        visited_i8s: Vec<i8>,
        visited_i16s: Vec<i16>,
        visited_i32s: Vec<i32>,
        visited_i64s: Vec<i64>,

        visited_f32s: Vec<f32>,
        visited_f64s: Vec<f64>,

        visited_strings: Vec<String>,

        visit_seq_times_called: u32,
        visited_byte_bufs: Vec<Vec<u8>>,
    }

    struct TestVisitor {
        state: Rc<RefCell<TestState>>,
    }

    impl TestVisitor {
        fn new() -> TestVisitor {
            TestVisitor { state: Rc::new(RefCell::new(TestState::default())) }
        }
    }

    impl<'de> Visitor<'de> for TestVisitor {
        type Value = ();

        fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(fmt, "SOME/IP primitive value")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_bools.push(v);
            Ok(())
        }

        fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_u8s.push(v);
            Ok(())
        }

        fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_u16s.push(v);
            Ok(())
        }

        fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_u32s.push(v);
            Ok(())
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_u64s.push(v);
            Ok(())
        }

        fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_i8s.push(v);
            Ok(())
        }

        fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_i16s.push(v);
            Ok(())
        }

        fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_i32s.push(v);
            Ok(())
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_i64s.push(v);
            Ok(())
        }

        fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_f32s.push(v);
            Ok(())
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_f64s.push(v);
            Ok(())
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_string(v.to_string())
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_strings.push(v);
            Ok(())
        }

        fn visit_seq<A>(self, _seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            self.state.borrow_mut().visit_seq_times_called += 1;
            Ok(())
        }

        fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.state.borrow_mut().visited_byte_bufs.push(v);
            Ok(())
        }
    }

    #[test]
    fn test_deserialize_bool_calls_visitor_correctly() {
        let data = vec![1];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_bool(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_bools.len(), 1);
        assert!(test_state.borrow().visited_bools[0]);
    }

    #[test]
    fn test_deserialize_u8_calls_visitor_correctly() {
        let value: u8 = 34;
        let data = vec![value];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_u8(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_u8s.len(), 1);
        assert_eq!(test_state.borrow().visited_u8s[0], value);
    }

    #[test]
    fn test_deserialize_u16_calls_visitor_correctly() {
        let value: u16 = 291;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_u16(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_u16s.len(), 1);
        assert_eq!(test_state.borrow().visited_u16s[0], value);
    }

    #[test]
    fn test_deserialize_u32_calls_visitor_correctly() {
        let value: u32 = 402312;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_u32(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_u32s.len(), 1);
        assert_eq!(test_state.borrow().visited_u32s[0], value);
    }

    #[test]
    fn test_deserialize_u64_calls_visitor_correctly() {
        let value: u64 = 279479479;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_u64(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_u64s.len(), 1);
        assert_eq!(test_state.borrow().visited_u64s[0], value);
    }

    #[test]
    fn test_deserialize_i8_calls_visitor_correctly() {
        let value: i8 = -34;
        let data = vec![value as u8];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_i8(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_i8s.len(), 1);
        assert_eq!(test_state.borrow().visited_i8s[0], value);
    }

    #[test]
    fn test_deserialize_i16_calls_visitor_correctly() {
        let value: i16 = -291;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_i16(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_i16s.len(), 1);
        assert_eq!(test_state.borrow().visited_i16s[0], value);
    }

    #[test]
    fn test_deserialize_i32_calls_visitor_correctly() {
        let value: i32 = -402312;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_i32(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_i32s.len(), 1);
        assert_eq!(test_state.borrow().visited_i32s[0], value);
    }

    #[test]
    fn test_deserialize_i64_calls_visitor_correctly() {
        let value: i64 = -279479479;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_i64(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_i64s.len(), 1);
        assert_eq!(test_state.borrow().visited_i64s[0], value);
    }

    #[test]
    fn test_deserialize_f32_calls_visitor_correctly() {
        let value: f32 = -36.78f32;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_f32(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_f32s.len(), 1);
        assert_eq!(test_state.borrow().visited_f32s[0], value);
    }

    #[test]
    fn test_deserialize_f64_calls_visitor_correctly() {
        let value: f64 = -23.51f64;
        let data = value.to_be_bytes();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_f64(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_f64s.len(), 1);
        assert_eq!(test_state.borrow().visited_f64s[0], value);
    }

    #[test]
    fn test_deserialize_string_calls_visitor_correctly() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_string(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_strings.len(), 1);
        assert_eq!(test_state.borrow().visited_strings[0], String::from("hello"));
    }

    #[test]
    fn test_deserialize_seq_calls_visitor_correctly() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_seq(visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visit_seq_times_called, 1);
    }

    #[test]
    fn test_deserialize_tuple_calls_visitor_correctly() {
        let data = vec![2, 5, 6];
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_tuple(data.len(), visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visited_byte_bufs.len(), 1);
        assert_eq!(test_state.borrow().visited_byte_bufs[0].len(), 3);
    }

    #[test]
    fn test_deserialize_tuple_struct_calls_visitor_correctly() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let test_state = visitor.state.clone();
        let result = deserializer.deserialize_tuple_struct("", data.len(), visitor);
        assert!(result.is_ok());
        assert_eq!(test_state.borrow().visit_seq_times_called, 1);
    }

    #[test]
    #[should_panic(expected = "deserialize_any should not be used for SOME/IP deserialization")]
    fn test_deserialize_any_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_any(visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_char should not be used for SOME/IP deserialization")]
    fn test_deserialize_char_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_char(visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_str should not be used for SOME/IP deserialization")]
    fn test_deserialize_str_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_str(visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_bytes should not be used for SOME/IP deserialization")]
    fn test_deserialize_bytes_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_bytes(visitor);
    }

    #[test]
    #[should_panic(
        expected = "deserialize_byte_buf should not be used for SOME/IP deserialization"
    )]
    fn test_deserialize_byte_buf_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_byte_buf(visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_option should not be used for SOME/IP deserialization")]
    fn test_deserialize_option_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_option(visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_unit should not be used for SOME/IP deserialization")]
    fn test_deserialize_unit_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_unit(visitor);
    }

    #[test]
    #[should_panic(
        expected = "deserialize_unit_struct should not be used for SOME/IP deserialization"
    )]
    fn test_deserialize_unit_struct_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_unit_struct("Test", visitor);
    }

    #[test]
    #[should_panic(
        expected = "deserialize_newtype_struct should not be used for SOME/IP deserialization"
    )]
    fn test_deserialize_newtype_struct_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_newtype_struct("Test", visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_map should not be used for SOME/IP deserialization")]
    fn test_deserialize_map_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_map(visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_struct should not be used for SOME/IP deserialization")]
    fn test_deserialize_struct_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_struct("Test", &[], visitor);
    }

    #[test]
    #[should_panic(expected = "deserialize_enum should not be used for SOME/IP deserialization")]
    fn test_deserialize_enum_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_enum("Test", &[], visitor);
    }

    #[test]
    #[should_panic(
        expected = "deserialize_identifier should not be used for SOME/IP deserialization"
    )]
    fn test_deserialize_identifier_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_identifier(visitor);
    }

    #[test]
    #[should_panic(
        expected = "deserialize_ignored_any should not be used for SOME/IP deserialization"
    )]
    fn test_deserialize_ignored_any_panics() {
        let data = Vec::new();
        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let visitor = TestVisitor::new();
        let _ = deserializer.deserialize_ignored_any(visitor);
    }
}
