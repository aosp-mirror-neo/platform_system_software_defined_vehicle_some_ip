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

use anyhow::bail;
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use someip_config::serde_config::ffi::StringType;
use std::borrow::Cow;

/// Byte order mark for UTF-8 encoded strings
const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];
/// Byte order mark for UTf-16 Big Endian encoded strings
const BOM_UTF16_BE: &[u8] = &[0xFE, 0xFF];
/// Byte order mark for UTf-16 Little Endian encoded strings
const BOM_UTF16_LE: &[u8] = &[0xFF, 0xFE];

pub(crate) trait ISomeIpReader<'input_lt> {
    /// Checks that there are at least `num_bytes` bytes that can be read using the
    /// unchecked methods
    fn available_for_unchecked_reads(&mut self, num_bytes: usize) -> anyhow::Result<()>;

    /// Reads a boolean from the input and advances the current index by 1
    fn read_bool(&mut self) -> anyhow::Result<bool>;

    /// Reads an i8 from the input and advances the current index by 1
    fn read_i8(&mut self) -> anyhow::Result<i8>;

    /// Reads a u8 from the input and advances the current index by 1
    fn read_u8(&mut self) -> anyhow::Result<u8>;

    /// Reads an i16 from the input and advances the current index by 2
    fn read_i16(&mut self) -> anyhow::Result<i16>;

    /// Reads an i32 from the input and advances the current index by 4
    fn read_i32(&mut self) -> anyhow::Result<i32>;

    /// Reads an i64 from the input and advances the current index by 8
    fn read_i64(&mut self) -> anyhow::Result<i64>;

    /// Reads a u16 from the input and advances the current index by 2
    fn read_u16(&mut self) -> anyhow::Result<u16>;

    /// Reads a u32 from the input and advances the current index by 4
    fn read_u32(&mut self) -> anyhow::Result<u32>;

    /// Reads a u64 from the input and advances the current index by 8
    fn read_u64(&mut self) -> anyhow::Result<u64>;

    /// Reads a f32 from the input and advances the current index by 4
    fn read_f32(&mut self) -> anyhow::Result<f32>;

    /// Reads a f64 from the input and advances the current index by 8
    fn read_f64(&mut self) -> anyhow::Result<f64>;

    /// Reads a boolean from the input and advances the current index by 1. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_bool_unchecked(&mut self) -> bool;

    /// Reads an i8 from the input and advances the current index by 1. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i8_unchecked(&mut self) -> i8;

    /// Reads a u8 from the input and advances the current index by 1. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u8_unchecked(&mut self) -> u8;

    /// Reads an i16 from the input and advances the current index by 2. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i16_unchecked(&mut self) -> i16;

    /// Reads an i32 from the input and advances the current index by 4. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i32_unchecked(&mut self) -> i32;

    /// Reads an i64 from the input and advances the current index by 8. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i64_unchecked(&mut self) -> i64;

    /// Reads a u16 from the input and advances the current index by 2. NOTE: This method
    /// // does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u16_unchecked(&mut self) -> u16;

    /// Reads a u32 from the input and advances the current index by 4. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u32_unchecked(&mut self) -> u32;

    /// Reads a u64 from the input and advances the current index by 8. NOTE: This method
    /// // does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u64_unchecked(&mut self) -> u64;

    /// Reads a f32 from the input and advances the current index by 4. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_f32_unchecked(&mut self) -> f32;

    /// Reads a f64 from the input and advances the current index by 8. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_f64_unchecked(&mut self) -> f64;

    /// Reads a SOME/IP formatted string from the input and advances the current index by the
    /// number of read bytes
    fn read_string(&mut self, string_size: usize) -> anyhow::Result<Cow<'input_lt, str>>;

    fn read_bytes_array(&mut self, size: usize) -> anyhow::Result<&'input_lt [u8]>;

    /// Returns the number of bytes read so far from the input
    fn num_read_bytes(&self) -> usize;

    /// Skips up to `num_bytes` bytes from the input. Returns the actual number of skipped bytes
    fn advance_by(&mut self, num_bytes: usize) -> usize;
}

pub(crate) struct SomeIpReader<'input_lt, B: ByteOrder> {
    input: &'input_lt [u8],
    index: usize,
    _pd: std::marker::PhantomData<B>,
}

impl<'input_lt, B: ByteOrder> SomeIpReader<'input_lt, B> {
    /// Creates a new `SomeIpReader` that deserializes data from the given `input`
    pub(crate) fn new(input: &'input_lt [u8]) -> Self {
        SomeIpReader { input, index: 0, _pd: std::marker::PhantomData }
    }

    pub(crate) fn remaining_bytes(&self) -> usize {
        self.input.len() - self.index
    }

    fn read_bom(&mut self) -> anyhow::Result<StringType> {
        if self.input[self.index] == BOM_UTF16_BE[0]
            && self.input[self.index + 1] == BOM_UTF16_BE[1]
        {
            self.index += 2;
            Ok(StringType::UTF_16_BE)
        } else if self.input[self.index] == BOM_UTF16_LE[0]
            && self.input[self.index + 1] == BOM_UTF16_LE[1]
        {
            self.index += 2;
            Ok(StringType::UTF_16_LE)
        } else if self.input[self.index] == BOM_UTF8[0]
            && self.input[self.index + 1] == BOM_UTF8[1]
            && self.input[self.index + 2] == BOM_UTF8[2]
        {
            self.index += 3;
            Ok(StringType::UTF_8)
        } else {
            bail!("Unknown BOM for string");
        }
    }
}

impl<'input_lt, B: ByteOrder> ISomeIpReader<'input_lt> for SomeIpReader<'input_lt, B> {
    fn available_for_unchecked_reads(&mut self, num_bytes: usize) -> anyhow::Result<()> {
        if num_bytes > (self.input.len() - self.index) {
            bail!("Attempted to read past end of input")
        } else {
            Ok(())
        }
    }

    /// Reads a boolean from the input and advances the current index by 1
    fn read_bool(&mut self) -> anyhow::Result<bool> {
        self.available_for_unchecked_reads(1)?;
        // safe because we checked index earlier
        Ok(self.read_bool_unchecked())
    }

    /// Reads an i8 from the input and advances the current index by 1
    fn read_i8(&mut self) -> anyhow::Result<i8> {
        self.available_for_unchecked_reads(1)?;
        // safe because we checked index earlier
        Ok(self.read_i8_unchecked())
    }

    /// Reads a u8 from the input and advances the current index by 1
    fn read_u8(&mut self) -> anyhow::Result<u8> {
        self.available_for_unchecked_reads(1)?;
        // safe because we checked index earlier
        Ok(self.read_u8_unchecked())
    }

    /// Reads an i16 from the input and advances the current index by 2
    fn read_i16(&mut self) -> anyhow::Result<i16> {
        self.available_for_unchecked_reads(2)?;
        // safe because we checked index earlier
        Ok(self.read_i16_unchecked())
    }
    /// Reads an i32 from the input and advances the current index by 4
    fn read_i32(&mut self) -> anyhow::Result<i32> {
        self.available_for_unchecked_reads(4)?;
        // safe because we checked index earlier
        Ok(self.read_i32_unchecked())
    }
    /// Reads an i64 from the input and advances the current index by 8
    fn read_i64(&mut self) -> anyhow::Result<i64> {
        self.available_for_unchecked_reads(8)?;
        // safe because we checked index earlier
        Ok(self.read_i64_unchecked())
    }

    /// Reads a u16 from the input and advances the current index by 2
    fn read_u16(&mut self) -> anyhow::Result<u16> {
        self.available_for_unchecked_reads(2)?;
        // safe because we checked index earlier
        Ok(self.read_u16_unchecked())
    }

    /// Reads a u32 from the input and advances the current index by 4
    fn read_u32(&mut self) -> anyhow::Result<u32> {
        self.available_for_unchecked_reads(4)?;
        // safe because we checked index earlier
        Ok(self.read_u32_unchecked())
    }

    /// Reads a u64 from the input and advances the current index by 8
    fn read_u64(&mut self) -> anyhow::Result<u64> {
        self.available_for_unchecked_reads(8)?;
        // safe because we checked index earlier
        Ok(self.read_u64_unchecked())
    }

    /// Reads a f32 from the input and advances the current index by 4
    fn read_f32(&mut self) -> anyhow::Result<f32> {
        self.available_for_unchecked_reads(4)?;
        // safe because we checked index earlier
        Ok(self.read_f32_unchecked())
    }

    /// Reads a f64 from the input and advances the current index by 8
    fn read_f64(&mut self) -> anyhow::Result<f64> {
        self.available_for_unchecked_reads(8)?;
        // safe because we checked index earlier
        Ok(self.read_f64_unchecked())
    }

    /// Reads a boolean from the input and advances the current index by 1. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_bool_unchecked(&mut self) -> bool {
        let value = self.input[self.index] != 0;
        self.index += 1;
        value
    }

    /// Reads an i8 from the input and advances the current index by 1. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i8_unchecked(&mut self) -> i8 {
        let value = self.input[self.index] as i8;
        self.index += 1;
        value
    }

    /// Reads a u8 from the input and advances the current index by 1. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u8_unchecked(&mut self) -> u8 {
        let value = self.input[self.index];
        self.index += 1;
        value
    }

    /// Reads an i16 from the input and advances the current index by 2. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i16_unchecked(&mut self) -> i16 {
        let value = B::read_i16(&self.input[self.index..]);
        self.index += 2;
        value
    }
    /// Reads an i32 from the input and advances the current index by 4. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i32_unchecked(&mut self) -> i32 {
        let value = B::read_i32(&self.input[self.index..]);
        self.index += 4;
        value
    }
    /// Reads an i64 from the input and advances the current index by 8. NOTE: This method
    /// // does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_i64_unchecked(&mut self) -> i64 {
        let value = B::read_i64(&self.input[self.index..]);
        self.index += 8;
        value
    }

    /// Reads a u16 from the input and advances the current index by 2. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u16_unchecked(&mut self) -> u16 {
        let value = B::read_u16(&self.input[self.index..]);
        self.index += 2;
        value
    }

    /// Reads a u32 from the input and advances the current index by 4. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u32_unchecked(&mut self) -> u32 {
        let value = B::read_u32(&self.input[self.index..]);
        self.index += 4;
        value
    }

    /// Reads a u64 from the input and advances the current index by 8. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_u64_unchecked(&mut self) -> u64 {
        let value = B::read_u64(&self.input[self.index..]);
        self.index += 8;
        value
    }

    /// Reads a f32 from the input and advances the current index by 4. NOTE: This method
    /// does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_f32_unchecked(&mut self) -> f32 {
        let value = B::read_f32(&self.input[self.index..]);
        self.index += 4;
        value
    }

    /// Reads a f64 from the input and advances the current index by 8. NOTE: This method
    /// // does not perform bound checking and so may panic. It is the responsibility of the caller
    /// to ensure that there is enough data in the buffer, by calling `available_for_unchecked_reads`
    fn read_f64_unchecked(&mut self) -> f64 {
        let value = B::read_f64(&self.input[self.index..]);
        self.index += 8;
        value
    }

    /// Reads a SOME/IP formatted string from the input and advances the current index by the
    /// number of read bytes
    fn read_string(&mut self, string_size: usize) -> anyhow::Result<Cow<'input_lt, str>> {
        // BOM for UTF16 strings is 2 bytes and correct delimiter is 2 null bytes = 4 bytes
        // BOM for UTF8 strings is 3 bytes and correct delimiter is 1 null byte = 4 byte
        self.available_for_unchecked_reads(4)?;
        let someip_string_type = self.read_bom()?;
        if someip_string_type == StringType::UTF_8 {
            let start_index = self.index;
            let string_data_length = string_size - 3;
            let Some(end_index) =
                self.input[start_index..].iter().take(string_data_length).position(|&x| x == 0)
            else {
                bail!("Did not find null byte within {} bytes of string", string_data_length);
            };
            self.index += end_index;
            // self.index now points to the null byte
            let string_slice = &self.input[start_index..self.index];
            let value = std::str::from_utf8(string_slice)
                .map_err(|_| anyhow::Error::msg("Unable to parse string from UTF8 bytes"))?;
            // consume the remaining bytes
            let remaining_bytes = string_data_length - (self.index - start_index);
            self.index += remaining_bytes;
            Ok(Cow::Borrowed(value))
        } else {
            // String is UTF-16
            let start_index = self.index;
            let string_data_length = string_size - 2;
            let end_index = if let Some(end_index) = self.input
                [start_index..start_index + string_data_length]
                .chunks(2)
                .position(|chunk| chunk.len() == 2 && chunk[0] == 0 && chunk[1] == 0)
            {
                // We found an even length UTF-16 string. The ending index in the original buffer
                // is the end_index we just found * 2, because we are iterating over chunks of size
                // 2
                end_index * 2
            } else {
                // We did not find an even length UTF-16 string
                // We still need to check if there is a UTF-16 string with an odd length
                let Some(end_index) = self.input[start_index..start_index + string_data_length]
                    .windows(2)
                    .position(|window| window[0] == 0 && window[1] == 0)
                else {
                    bail!("Did not find null bytes within {} bytes of string", string_data_length);
                };
                // we return end_index - 1 since we need to discard the last byte before the null characters
                end_index - 1
            };
            self.index += end_index;
            // self.index now points to the first byte after the string data
            let string_len = self.index - start_index;
            let mut string_bytes = vec![0; string_len / 2];
            if !string_len.is_multiple_of(2) {
                // We should never hit this case since we checked already
                bail!("Internal error: UTF-16 string length is not even")
            }
            if someip_string_type == StringType::UTF_16_BE {
                BigEndian::read_u16_into(&self.input[start_index..self.index], &mut string_bytes);
            } else {
                LittleEndian::read_u16_into(
                    &self.input[start_index..self.index],
                    &mut string_bytes,
                );
            }
            let text = String::from_utf16(&string_bytes)
                .map_err(|_| anyhow::Error::msg("Unable to parse string from UTF16 bytes"))?;
            // consume the remaining bytes
            let remaining_bytes = string_data_length - (self.index - start_index);
            self.index += remaining_bytes;
            Ok(Cow::Owned(text))
        }
    }

    fn read_bytes_array(&mut self, size: usize) -> anyhow::Result<&'input_lt [u8]> {
        self.available_for_unchecked_reads(size)?;
        let value = &self.input[self.index..self.index + size];
        self.index += size;
        Ok(value)
    }

    /// Returns the number of bytes read so far from the input
    fn num_read_bytes(&self) -> usize {
        self.index
    }

    /// Skips up to `num_bytes` bytes from the input. Returns the actual number of skipped bytes
    fn advance_by(&mut self, num_bytes: usize) -> usize {
        let skipped_bytes = std::cmp::min(num_bytes, self.input.len() - self.index);
        self.index += skipped_bytes;
        skipped_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn test_new() {
        let reader = SomeIpReader::<BigEndian>::new(&[1, 2, 3]);
        assert_eq!(reader.index, 0);
        assert_eq!(reader.input, &[1, 2, 3]);
    }

    #[test]
    fn test_num_read_bytes() {
        let mut reader = SomeIpReader::<BigEndian>::new(&[1, 2, 3, 4]);
        assert_eq!(reader.num_read_bytes(), 0);
        reader.read_u16().unwrap();
        assert_eq!(reader.num_read_bytes(), 2);
        reader.read_u16().unwrap();
        assert_eq!(reader.num_read_bytes(), 4);
    }

    #[test]
    fn test_read_past_end() {
        let mut reader = SomeIpReader::<BigEndian>::new(&[1, 2]);
        let result = reader.read_u32();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Attempted to read past end of input");
    }

    #[test]
    fn test_read_bool() {
        let mut reader = SomeIpReader::<BigEndian>::new(&[1, 0, 42]);
        assert!(reader.read_bool().unwrap());
        assert!(!reader.read_bool().unwrap());
        assert!(reader.read_bool().unwrap());
    }

    #[test]
    fn test_read_u8() {
        let mut reader = SomeIpReader::<BigEndian>::new(&[123]);
        assert_eq!(reader.read_u8().unwrap(), 123);
    }

    #[test]
    fn test_read_i8() {
        let mut reader = SomeIpReader::<BigEndian>::new(&[-50i8 as u8]);
        assert_eq!(reader.read_i8().unwrap(), -50);
    }

    macro_rules! test_primitive_read {
        ($test_name:ident, $read_method:ident, $write_method:ident, $ty:ty, $value:expr) => {
            #[test]
            fn $test_name() {
                const SIZE: usize = std::mem::size_of::<$ty>();
                let mut buf = [0; SIZE];

                // Big Endian
                BigEndian::$write_method(&mut buf, $value);
                let mut reader_be = SomeIpReader::<BigEndian>::new(&buf);
                assert_eq!(reader_be.$read_method().unwrap(), $value);
                assert_eq!(reader_be.num_read_bytes(), SIZE);

                // Little Endian
                LittleEndian::$write_method(&mut buf, $value);
                let mut reader_le = SomeIpReader::<LittleEndian>::new(&buf);
                assert_eq!(reader_le.$read_method().unwrap(), $value);
                assert_eq!(reader_le.num_read_bytes(), SIZE);
            }
        };
    }

    test_primitive_read!(test_read_u16, read_u16, write_u16, u16, 0xABCD);
    test_primitive_read!(test_read_i16, read_i16, write_i16, i16, -0x1234);
    test_primitive_read!(test_read_u32, read_u32, write_u32, u32, 0x1234_5678);
    test_primitive_read!(test_read_i32, read_i32, write_i32, i32, -0x1234_5678);
    test_primitive_read!(test_read_u64, read_u64, write_u64, u64, 0x1234_5678_9ABC_DEF0);
    test_primitive_read!(test_read_i64, read_i64, write_i64, i64, -0x1234_5678_9ABC_DEF0);
    test_primitive_read!(test_read_f32, read_f32, write_f32, f32, 123.456);
    test_primitive_read!(test_read_f64, read_f64, write_f64, f64, -123.456789);

    #[test]
    fn test_read_string_utf8() {
        let mut buf = Vec::new();
        buf.extend_from_slice(BOM_UTF8); // 3 bytes
        buf.extend_from_slice(b"hello"); // 5 bytes
        buf.push(0); // 1 byte null terminator
        buf.extend_from_slice(&[0, 0, 0]); // 3 bytes padding

        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        let result = reader.read_string(buf.len()).unwrap();
        assert_eq!(result, Cow::Borrowed("hello"));
        assert_eq!(reader.num_read_bytes(), buf.len());
    }

    #[test]
    fn test_read_string_utf16_be() {
        let mut buf = Vec::new();
        buf.extend_from_slice(BOM_UTF16_BE); // 2 bytes
        let s: Vec<u16> = "world".encode_utf16().collect();
        for c in &s {
            buf.extend_from_slice(&c.to_be_bytes());
        }
        buf.extend_from_slice(&[0, 0]); // 2 bytes null terminator
        buf.extend_from_slice(&[0, 0]); // 2 bytes padding

        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        let result = reader.read_string(buf.len()).unwrap();
        assert_eq!(result, Cow::Owned::<str>("world".to_string()));
        assert_eq!(reader.num_read_bytes(), buf.len());
    }

    #[test]
    fn test_read_string_utf16_le() {
        let mut buf = Vec::new();
        buf.extend_from_slice(BOM_UTF16_LE); // 2 bytes
        let s: Vec<u16> = "world".encode_utf16().collect();
        for c in &s {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        buf.extend_from_slice(&[0, 0]); // 2 bytes null terminator
        let mut reader = SomeIpReader::<LittleEndian>::new(&buf);
        let result = reader.read_string(buf.len()).unwrap();
        assert_eq!(result, Cow::Owned::<str>("world".to_string()));
        assert_eq!(reader.num_read_bytes(), buf.len());
    }

    #[test]
    fn test_read_string_unknown_bom() {
        let buf = [0x12, 0x34, 0x56, 0x78];
        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        let result = reader.read_string(4);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unknown BOM for string");
    }

    #[test]
    fn test_read_string_no_null_terminator_utf8() {
        let mut buf = Vec::new();
        buf.extend_from_slice(BOM_UTF8);
        buf.extend_from_slice(b"hello");

        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        let result = reader.read_string(buf.len());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Did not find null byte within 5 bytes of string"
        );
    }

    #[test]
    fn test_read_string_no_null_terminator_utf16() {
        let mut buf = Vec::new();
        buf.extend_from_slice(BOM_UTF16_BE);
        buf.extend_from_slice(&[0, b'h', 0, b'i']);

        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        let result = reader.read_string(buf.len());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Did not find null bytes within 4 bytes of string"
        );
    }

    #[test]
    fn test_read_string_utf16_odd_length() {
        let mut buf = Vec::new();
        buf.extend_from_slice(BOM_UTF16_BE);

        let s: Vec<u16> = "world".encode_utf16().collect();
        for c in &s {
            buf.extend_from_slice(&c.to_be_bytes());
        }
        buf.push(b'b'); // Extra byte that should be ignored
        buf.extend_from_slice(&[0, 0]); // 2 bytes null terminator

        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        let result = reader.read_string(buf.len()).unwrap();
        assert_eq!(result, Cow::Owned::<str>("world".to_string()));
        assert_eq!(reader.num_read_bytes(), buf.len());
    }

    #[test]
    fn test_read_bytes_array() {
        let buf = [1, 2, 3, 4, 5];
        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        assert_eq!(reader.read_bytes_array(3).unwrap(), &[1, 2, 3]);
        assert_eq!(reader.num_read_bytes(), 3);
        assert_eq!(reader.read_bytes_array(2).unwrap(), &[4, 5]);
        assert_eq!(reader.num_read_bytes(), 5);
    }

    #[test]
    fn test_read_bytes_array_past_end() {
        let buf = [1, 2, 3];
        let mut reader = SomeIpReader::<BigEndian>::new(&buf);
        let result = reader.read_bytes_array(4);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Attempted to read past end of input");
    }

    #[test]
    fn test_advance_by() {
        let mut reader = SomeIpReader::<BigEndian>::new(&[1, 2, 3, 4, 5]);
        assert_eq!(reader.num_read_bytes(), 0);

        // Advance by zero
        let advanced = reader.advance_by(0);
        assert_eq!(advanced, 0);
        assert_eq!(reader.num_read_bytes(), 0);

        // Advance within bounds
        let advanced = reader.advance_by(2);
        assert_eq!(advanced, 2);
        assert_eq!(reader.num_read_bytes(), 2);

        // Advance past the end, should only advance by remaining bytes
        let advanced = reader.advance_by(10);
        assert_eq!(advanced, 3); // 5 - 2 = 3 bytes left
        assert_eq!(reader.num_read_bytes(), 5);

        // Try to advance when at the end
        let advanced = reader.advance_by(1);
        assert_eq!(advanced, 0);
        assert_eq!(reader.num_read_bytes(), 5);
    }
}
