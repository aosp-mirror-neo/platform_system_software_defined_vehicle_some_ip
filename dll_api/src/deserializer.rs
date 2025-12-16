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
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
pub use someip_config::serde_config::ffi::StringType;

/// Byte order mark for UTF-8 encoded strings
const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Byte order mark for UTf-16 Big Endian encoded strings
const BOM_UTF16_BE: &[u8] = &[0xFE, 0xFF];

/// Byte order mark for UTf-16 Little Endian encoded strings
const BOM_UTF16_LE: &[u8] = &[0xFF, 0xFE];

/// Deserializer for SOME/IP wire format. The deserializer holds an immutable reference to the
/// input data and maintains a current index from which the next primitive type should be read
pub struct SomeIpDeserializer<'input_lt, B: ByteOrder> {
    input: &'input_lt [u8],
    index: usize,
    _pd: std::marker::PhantomData<B>,
}

/// Trait for SOME/IP wire format deserializers
pub trait ISomeIpDeserializer {
    /// Read a boolean from SOME/IP format
    fn deserialize_bool(&mut self) -> SdvResult<bool>;

    /// Reads an i8 from SOME/IP format
    fn deserialize_i8(&mut self) -> SdvResult<i8>;

    /// Reads a u8 from SOME/IP format
    fn deserialize_u8(&mut self) -> SdvResult<u8>;

    /// Reads an i16 from SOME/IP format
    fn deserialize_i16(&mut self) -> SdvResult<i16>;

    /// Reads an i32 from SOME/IP format
    fn deserialize_i32(&mut self) -> SdvResult<i32>;

    /// Reads an i64 from SOME/IP format
    fn deserialize_i64(&mut self) -> SdvResult<i64>;

    /// Reads a u16 from SOME/IP format
    fn deserialize_u16(&mut self) -> SdvResult<u16>;

    /// Reads a u32 from SOME/IP format
    fn deserialize_u32(&mut self) -> SdvResult<u32>;

    /// Reads a u64 from SOME/IP format
    fn deserialize_u64(&mut self) -> SdvResult<u64>;

    /// Reads a f32 from SOME/IP format
    fn deserialize_f32(&mut self) -> SdvResult<f32>;

    /// Reads a f64 from SOME/IP format
    fn deserialize_f64(&mut self) -> SdvResult<f64>;

    /// Reads a SOME/IP formatted string
    fn deserialize_string(&mut self) -> SdvResult<String>;

    /// Reads the specified number of bytes
    fn read_bytes(&mut self, bytes_to_read: usize) -> SdvResult<Vec<u8>>;

    /// Returns the number of bytes read so far by the deserializer
    fn num_read_bytes(&self) -> usize;
}

impl<'input_lt, B: ByteOrder> SomeIpDeserializer<'input_lt, B> {
    /// Creates a new `SomeIpDeserializer` that deserializes data from the given `input`
    pub fn new(input: &'input_lt [u8]) -> Self {
        SomeIpDeserializer { input, index: 0, _pd: std::marker::PhantomData }
    }

    #[inline]
    fn check_index(&self, num_bytes: usize) -> SdvResult<()> {
        if num_bytes > (self.input.len() - self.index) {
            Err(SdvStatus::new(SdvStatusCode::OutOfRange))
        } else {
            Ok(())
        }
    }
}

impl<B: ByteOrder> ISomeIpDeserializer for SomeIpDeserializer<'_, B> {
    /// Reads a boolean from the input and advances the current index by 1
    fn deserialize_bool(&mut self) -> SdvResult<bool> {
        self.check_index(1)?;
        // safe because we checked index earlier
        let value = self.input[self.index];
        let boolean_value = value != 0;
        self.index += 1;
        Ok(boolean_value)
    }

    /// Reads an i8 from the input and advances the current index by 1
    fn deserialize_i8(&mut self) -> SdvResult<i8> {
        self.check_index(1)?;
        // safe because we checked index earlier
        let value = self.input[self.index] as i8;
        self.index += 1;
        Ok(value)
    }

    /// Reads a u8 from the input and advances the current index by 1
    fn deserialize_u8(&mut self) -> SdvResult<u8> {
        self.check_index(1)?;
        // safe because we checked index earlier
        let value = self.input[self.index];
        self.index += 1;
        Ok(value)
    }

    /// Reads an i16 from the input and advances the current index by 2
    fn deserialize_i16(&mut self) -> SdvResult<i16> {
        self.check_index(2)?;
        // safe because we checked index earlier
        let value = B::read_i16(&self.input[self.index..]);
        self.index += 2;
        Ok(value)
    }

    /// Reads an i32 from the input and advances the current index by 4
    fn deserialize_i32(&mut self) -> SdvResult<i32> {
        self.check_index(4)?;
        // safe because we checked index earlier
        let value = B::read_i32(&self.input[self.index..]);
        self.index += 4;
        Ok(value)
    }

    /// Reads an i64 from the input and advances the current index by 8
    fn deserialize_i64(&mut self) -> SdvResult<i64> {
        self.check_index(8)?;
        // safe because we checked index earlier
        let value = B::read_i64(&self.input[self.index..]);
        self.index += 8;
        Ok(value)
    }

    /// Reads a u16 from the input and advances the current index by 2
    fn deserialize_u16(&mut self) -> SdvResult<u16> {
        self.check_index(2)?;
        // safe because we checked index earlier
        let value = B::read_u16(&self.input[self.index..]);
        self.index += 2;
        Ok(value)
    }

    /// Reads a u32 from the input and advances the current index by 4
    fn deserialize_u32(&mut self) -> SdvResult<u32> {
        self.check_index(4)?;
        // safe because we checked index earlier
        let value = B::read_u32(&self.input[self.index..]);
        self.index += 4;
        Ok(value)
    }

    /// Reads a u64 from the input and advances the current index by 8
    fn deserialize_u64(&mut self) -> SdvResult<u64> {
        self.check_index(8)?;
        // safe because we checked index earlier
        let value = B::read_u64(&self.input[self.index..]);
        self.index += 8;
        Ok(value)
    }

    /// Reads a f32 from the input and advances the current index by 4
    fn deserialize_f32(&mut self) -> SdvResult<f32> {
        self.check_index(4)?;
        // safe because we checked index earlier
        let value = B::read_f32(&self.input[self.index..]);
        self.index += 4;
        Ok(value)
    }

    /// Reads a f64 from the input and advances the current index by 8
    fn deserialize_f64(&mut self) -> SdvResult<f64> {
        self.check_index(8)?;
        // safe because we checked index earlier
        let value = B::read_f64(&self.input[self.index..]);
        self.index += 8;
        Ok(value)
    }

    /// Reads a SOME/IP formatted string from the input and advances the current index by the
    /// number of read bytes
    fn deserialize_string(&mut self) -> SdvResult<String> {
        // BOM for UTF16 strings is 2 bytes and correct delimiter is 2 null bytes = 4 bytes
        // BOM for UTF8 strings is 3 bytes and correct delimiter is 1 null byte = 4 byte
        self.check_index(4)?;

        let someip_string_type = if self.input[self.index] == BOM_UTF16_BE[0]
            && self.input[self.index + 1] == BOM_UTF16_BE[1]
        {
            StringType::UTF_16_BE
        } else if self.input[self.index] == BOM_UTF16_LE[0]
            && self.input[self.index + 1] == BOM_UTF16_LE[1]
        {
            StringType::UTF_16_LE
        } else if self.input[self.index] == BOM_UTF8[0]
            && self.input[self.index + 1] == BOM_UTF8[1]
            && self.input[self.index + 2] == BOM_UTF8[2]
        {
            StringType::UTF_8
        } else {
            return Err(SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                "Unknown BOM for string",
            ));
        };

        if someip_string_type == StringType::UTF_8 {
            self.index += 3;
            let start_index = self.index;
            while self.index < self.input.len() {
                if self.input[self.index] == 0x00 {
                    break;
                }
                self.index += 1;
            }
            if self.index != self.input.len() {
                let string_bytes = self.input[start_index..self.index].to_vec();
                let text = String::from_utf8(string_bytes).map_err(|_| {
                    SdvStatus::with_message(
                        SdvStatusCode::InvalidArgument,
                        "Unable to parse string from UTF8 bytes",
                    )
                })?;
                self.index += 1; // consume the null byte
                Ok(text)
            } else {
                Err(SdvStatus::new(SdvStatusCode::OutOfRange))
            }
        } else {
            // String is UTF16
            self.index += 2;
            let start_index = self.index;
            while self.index + 1 < self.input.len() {
                if self.input[self.index] == 0x00 && self.input[self.index + 1] == 0x00 {
                    break;
                }
                self.index += 2;
            }
            if self.index + 1 != self.input.len() {
                let string_len = self.index - start_index;
                let mut string_bytes = vec![0; string_len / 2];
                if !string_len.is_multiple_of(2) {
                    return Err(SdvStatus::new(SdvStatusCode::Internal));
                }
                if someip_string_type == StringType::UTF_16_BE {
                    BigEndian::read_u16_into(
                        &self.input[start_index..self.index],
                        &mut string_bytes,
                    );
                } else {
                    LittleEndian::read_u16_into(
                        &self.input[start_index..self.index],
                        &mut string_bytes,
                    );
                }
                let text = String::from_utf16(&string_bytes).map_err(|_| {
                    SdvStatus::with_message(
                        SdvStatusCode::Internal,
                        "Unable to parse string from UTF16 bytes",
                    )
                })?;
                self.index += 2; // consume the 2 null bytes
                Ok(text)
            } else {
                Err(SdvStatus::new(SdvStatusCode::OutOfRange))
            }
        }
    }

    /// Reads the specified number of bytes and advances the index by the number of bytes read
    fn read_bytes(&mut self, bytes_to_read: usize) -> SdvResult<Vec<u8>> {
        self.check_index(bytes_to_read)?;
        let byte_data = self.input[self.index..(self.index + bytes_to_read)].to_vec();
        self.index += bytes_to_read;
        Ok(byte_data)
    }

    /// Returns the number of bytes read so far from the input
    fn num_read_bytes(&self) -> usize {
        self.index
    }
}

#[cfg(test)]
mod tests {
    use sdv_status_rs::SdvStatusCode;
    use someip_config::serde_config::ffi::Endianness;

    use crate::create_someip_deserializer;
    use crate::deserializer::{BOM_UTF16_BE, BOM_UTF16_LE, BOM_UTF8};

    #[test]
    fn test_deserialize_false_bool_succeeds() {
        let value = 0;
        let data = vec![value];
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");
        assert_eq!(deserializer.num_read_bytes(), 0);
        assert!(!deserializer.deserialize_bool().expect("Deserializing bool failed"));
        assert_eq!(deserializer.num_read_bytes(), 1);
    }

    #[test]
    fn test_deserialize_true_bool_succeeds() {
        let value = 1;
        let data = vec![value];
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");
        assert_eq!(deserializer.num_read_bytes(), 0);
        assert!(deserializer.deserialize_bool().expect("Deserializing bool failed"));
        assert_eq!(deserializer.num_read_bytes(), 1);
    }

    #[test]
    fn test_deserialize_big_endian_u8_succeeds() {
        let value = 4;
        let data = vec![value];
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");
        assert_eq!(deserializer.num_read_bytes(), 0);
        assert_eq!(deserializer.deserialize_u8().expect("Deserializing u8 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 1);
    }

    #[test]
    fn test_deserialize_big_endian_u16_succeeds() {
        let value: u16 = 905;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_u16().expect("Deserializing u16 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 2);
    }

    #[test]
    fn test_deserialize_big_endian_u32_succeeds() {
        let value: u32 = 782091;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_u32().expect("Deserializing u32 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 4);
    }

    #[test]
    fn test_deserialize_big_endian_u64_succeeds() {
        let value: u64 = 7219373826;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_u64().expect("Deserializing u64 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 8);
    }

    #[test]
    fn test_deserialize_big_endian_i16_succeeds() {
        let value: i16 = -905;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_i16().expect("Deserializing i16 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 2);
    }

    #[test]
    fn test_deserialize_big_endian_i32_succeeds() {
        let value: i32 = -782091;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_i32().expect("Deserializing i32 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 4);
    }

    #[test]
    fn test_deserialize_big_endian_i64_succeeds() {
        let value: i64 = -72373826;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_i64().expect("Deserializing i64 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 8);
    }

    #[test]
    fn test_deserialize_big_endian_f32_succeeds() {
        let value: f32 = 4.95f32;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_f32().expect("Deserializing f32 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 4);
    }

    #[test]
    fn test_deserialize_big_endian_f64_succeeds() {
        let value: f64 = -72376f64;
        let data = value.to_be_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_f64().expect("Deserializing f64 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 8);
    }

    #[test]
    fn test_deserialize_little_endian_u8_succeeds() {
        let value = 4;
        let data = vec![value];
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");
        assert_eq!(deserializer.num_read_bytes(), 0);
        assert_eq!(deserializer.deserialize_u8().expect("Deserializing u8 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 1);
    }

    #[test]
    fn test_deserialize_little_endian_u16_succeeds() {
        let value: u16 = 905;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_u16().expect("Deserializing u16 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 2);
    }

    #[test]
    fn test_deserialize_little_endian_u32_succeeds() {
        let value: u32 = 782091;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_u32().expect("Deserializing u32 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 4);
    }

    #[test]
    fn test_deserialize_little_endian_u64_succeeds() {
        let value: u64 = 7219373826;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_u64().expect("Deserializing u64 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 8);
    }

    #[test]
    fn test_deserialize_little_endian_i16_succeeds() {
        let value: i16 = -905;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_i16().expect("Deserializing i16 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 2);
    }

    #[test]
    fn test_deserialize_little_endian_i32_succeeds() {
        let value: i32 = -782091;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_i32().expect("Deserializing i32 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 4);
    }

    #[test]
    fn test_deserialize_little_endian_i64_succeeds() {
        let value: i64 = -72373826;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_i64().expect("Deserializing i64 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 8);
    }

    #[test]
    fn test_deserialize_little_endian_f32_succeeds() {
        let value: f32 = 4.95f32;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_f32().expect("Deserializing f32 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 4);
    }

    #[test]
    fn test_deserialize_little_endian_f64_succeeds() {
        let value: f64 = -72376f64;
        let data = value.to_le_bytes();
        let mut deserializer = create_someip_deserializer(&data, Endianness::LittleEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_f64().expect("Deserializing f64 failed"), value);
        assert_eq!(deserializer.num_read_bytes(), 8);
    }

    #[test]
    fn test_deserialize_big_endian_series_succeeds() {
        let u16_value: u16 = 24;
        let u16_data = u16_value.to_be_bytes();

        let f64_value: f64 = 879.61f64;
        let f64_data = f64_value.to_be_bytes();

        let u8_value: u8 = 24;
        let u8_data = u8_value.to_be_bytes();

        let mut data = Vec::new();
        data.extend_from_slice(&u16_data);
        data.extend_from_slice(&f64_data);
        data.extend_from_slice(&u8_data);

        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_u16().expect("Deserializing u16 failed"), u16_value);
        assert_eq!(deserializer.num_read_bytes(), 2);
        assert_eq!(deserializer.deserialize_f64().expect("Deserializing f64 failed"), f64_value);
        assert_eq!(deserializer.num_read_bytes(), 10);
        assert_eq!(deserializer.deserialize_u8().expect("Deserializing u8 failed"), u8_value);
        assert_eq!(deserializer.num_read_bytes(), 11);
    }

    #[test]
    fn test_deserialize_utf8_string_succeeds() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_string().expect("Deserializing string failed"), text);
        assert_eq!(deserializer.num_read_bytes(), 3 + 1 + 5);
    }

    #[test]
    fn test_deserialize_utf16be_string_succeeds() {
        let text = String::from("hello");
        let text_u16_rep: Vec<u16> = text.encode_utf16().collect();
        let mut text_bytes = Vec::new();
        for v in text_u16_rep {
            text_bytes.push((v >> 8) as u8);
            text_bytes.push((v & 0xFF) as u8);
        }

        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF16_BE);
        data.extend_from_slice(&text_bytes);
        data.push(0x00);
        data.push(0x00);
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_string().expect("Deserializing string failed"), text);
        assert_eq!(deserializer.num_read_bytes(), 2 + 2 + 10);
    }

    #[test]
    fn test_deserialize_utf16le_string_succeeds() {
        let text = String::from("hello");
        let text_u16_rep: Vec<u16> = text.encode_utf16().collect();
        let mut text_bytes = Vec::new();
        for v in text_u16_rep {
            text_bytes.push((v & 0xFF) as u8);
            text_bytes.push((v >> 8) as u8);
        }

        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF16_LE);
        data.extend_from_slice(&text_bytes);
        data.push(0x00);
        data.push(0x00);
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        assert_eq!(deserializer.deserialize_string().expect("Deserializing string failed"), text);
        assert_eq!(deserializer.num_read_bytes(), 2 + 2 + 10);
    }

    #[test]
    fn test_deserialize_utf16_string_incorrectly_terminated_fails() {
        let text = String::from("hello");
        let text_u16_rep: Vec<u16> = text.encode_utf16().collect();
        let mut text_bytes = Vec::new();
        for v in text_u16_rep {
            text_bytes.push((v & 0xFF) as u8);
            text_bytes.push((v >> 8) as u8);
        }

        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF16_LE);
        data.extend_from_slice(&text_bytes);
        data.push(0x00);
        let mut deserializer = create_someip_deserializer(&data, Endianness::BigEndian)
            .expect("SOME/IP deserializer must be created successfully");

        let result = deserializer.deserialize_string();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status_code(), SdvStatusCode::OutOfRange);
    }
}
