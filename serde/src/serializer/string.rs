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

use byteorder::{BigEndian, LittleEndian, WriteBytesExt};

use protobuf::reflect::ReflectValueRef;
use someip_config::serde_config::ffi::StringType;

use serde::ser::{Error, SerializeSeq};
use serde::Serialize;

use crate::serializer::get_byte_size;
use someip_serde_reflect::SomeIpStringTypeDefinition;

/// Byte order mark for UTF-8 encoded strings
pub(crate) const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];
/// Byte order mark for UTf-16 Big Endian encoded strings
pub(crate) const BOM_UTF16_BE: &[u8] = &[0xFE, 0xFF];
/// Byte order mark for UTf-16 Little Endian encoded strings
pub(crate) const BOM_UTF16_LE: &[u8] = &[0xFF, 0xFE];

pub(crate) struct StringValueSeed<'lt> {
    someip_string_definition: &'lt SomeIpStringTypeDefinition,
    proto_field_value: ReflectValueRef<'lt>,
}

impl<'lt> StringValueSeed<'lt> {
    pub(crate) fn new(
        someip_string_definition: &'lt SomeIpStringTypeDefinition,
        proto_field_value: ReflectValueRef<'lt>,
    ) -> Self {
        Self { someip_string_definition, proto_field_value }
    }
}

impl Serialize for StringValueSeed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let ReflectValueRef::String(str_value) = self.proto_field_value {
            if self.someip_string_definition.fixed_length.is_some() {
                StringBytesSeed::new(self.someip_string_definition, str_value).serialize(serializer)
            } else {
                // reserve space for string length
                let reserved_space =
                    get_byte_size(&self.someip_string_definition.length_field_size)
                        .map_err(|e| S::Error::custom(e.to_string()))?;
                let mut seq = serializer.serialize_seq(Some(reserved_space))?;
                seq.serialize_element(&StringBytesSeed::new(
                    self.someip_string_definition,
                    str_value,
                ))?;
                seq.end()
            }
        } else {
            Err(S::Error::custom("Expected a string proto value"))
        }
    }
}

pub(crate) struct StringBytesSeed<'lt> {
    someip_string_definition: &'lt SomeIpStringTypeDefinition,
    str_value: &'lt str,
}

impl<'lt> StringBytesSeed<'lt> {
    pub(crate) fn new(
        someip_string_definition: &'lt SomeIpStringTypeDefinition,
        str_value: &'lt str,
    ) -> Self {
        Self { someip_string_definition, str_value }
    }
}

impl Serialize for StringBytesSeed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serialized_string_bytes = Vec::new();
        match self.someip_string_definition.bom {
            StringType::UTF_8 => {
                serialized_string_bytes.extend_from_slice(BOM_UTF8);
                serialized_string_bytes.extend_from_slice(self.str_value.as_bytes());
                serialized_string_bytes.push(0x00);
            }
            StringType::UTF_16_BE => {
                let mut serialized_string: Vec<u16> = self.str_value.encode_utf16().collect();
                serialized_string.push(0);

                serialized_string_bytes.extend_from_slice(BOM_UTF16_BE);
                for blob in serialized_string {
                    serialized_string_bytes
                        .write_u16::<BigEndian>(blob)
                        .map_err(|e| S::Error::custom(e.to_string()))?;
                }
            }
            StringType::UTF_16_LE => {
                let mut serialized_string: Vec<u16> = self.str_value.encode_utf16().collect();
                serialized_string.push(0);

                serialized_string_bytes.extend_from_slice(BOM_UTF16_LE);
                for blob in serialized_string {
                    serialized_string_bytes
                        .write_u16::<LittleEndian>(blob)
                        .map_err(|e| S::Error::custom(e.to_string()))?;
                }
            }
            _ => return Err(S::Error::custom("Invalid string type".to_string())),
        }

        if let Some(fixed_len) = self.someip_string_definition.fixed_length {
            if serialized_string_bytes.len() > fixed_len {
                return Err(S::Error::custom(format!(
                    "Serialized string (BOM + content + null terminator) length {} exceeds fixed length {}",
                    serialized_string_bytes.len(),
                    fixed_len
                )));
            }
            if serialized_string_bytes.len() < fixed_len {
                serialized_string_bytes.resize(fixed_len, 0x00);
            }
        }
        serializer.serialize_bytes(&serialized_string_bytes)
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{BigEndian, LittleEndian};
    use protobuf::reflect::ReflectValueRef;

    use serde::Serialize;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};

    use crate::serializer::serializer::SomeIpSerdeSerializer;
    use crate::serializer::string::StringValueSeed;
    use someip_serde_reflect::SomeIpStringTypeDefinition;

    use super::{BOM_UTF16_BE, BOM_UTF16_LE, BOM_UTF8};

    const SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::OneByte,
    };

    #[test]
    fn test_dynamic_length_string_serialize_succeeds() {
        let text = String::from("hello");

        let mut serializer = SomeIpSerdeSerializer::<LittleEndian>::new(SOMEIP_CONFIG);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: None,
        };
        let proto_field_value = ReflectValueRef::String(&text);

        let string_value_seed = StringValueSeed::new(&string_type_definition, proto_field_value);

        string_value_seed.serialize(&mut serializer).expect("Serialization must succeed");

        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.push(3 + 5 + 1);
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);
        assert_eq!(serializer.get_serialized_bytes(), data);
    }

    #[test]
    fn test_fixed_length_string_serialize_succeeds() {
        let text = String::from("hello");

        let mut serializer = SomeIpSerdeSerializer::<LittleEndian>::new(SOMEIP_CONFIG);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: Some(3 + 5 + 1),
        };
        let proto_field_value = ReflectValueRef::String(&text);

        let string_value_seed = StringValueSeed::new(&string_type_definition, proto_field_value);

        string_value_seed.serialize(&mut serializer).expect("Serialization must succeed");

        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        assert_eq!(serializer.get_serialized_bytes(), data);
    }

    #[test]
    fn test_fixed_length_string_with_short_actual_data_serialize_succeeds() {
        let text = String::from("hello");

        let mut serializer = SomeIpSerdeSerializer::<LittleEndian>::new(SOMEIP_CONFIG);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: Some(3 + 5 + 1 + /*extera length*/10),
        };
        let proto_field_value = ReflectValueRef::String(&text);

        let string_value_seed = StringValueSeed::new(&string_type_definition, proto_field_value);

        string_value_seed.serialize(&mut serializer).expect("Serialization must succeed");

        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);
        data.extend_from_slice(&[0u8; 10]);

        assert_eq!(serializer.get_serialized_bytes(), data);
    }

    #[test]
    fn test_string_serialize_with_no_length_fails() {
        let text = String::from("hello");

        let mut serializer = SomeIpSerdeSerializer::<LittleEndian>::new(SOMEIP_CONFIG);
        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::ZeroBytes,
            bom: StringType::UTF_8,
            fixed_length: None,
        };
        let proto_field_value = ReflectValueRef::String(&text);
        let string_value_seed = StringValueSeed::new(&string_type_definition, proto_field_value);
        let result = string_value_seed.serialize(&mut serializer);

        assert!(result.is_err());
    }

    #[test]
    fn test_fixed_length_string_with_long_actual_data_serialize_fails() {
        let text = String::from("hello world"); // This string is longer

        let mut serializer = SomeIpSerdeSerializer::<LittleEndian>::new(SOMEIP_CONFIG);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: Some(3 + 5 + 1), // Fixed length for "hello" + BOM + null
        };
        let proto_field_value = ReflectValueRef::String(&text);

        let string_value_seed = StringValueSeed::new(&string_type_definition, proto_field_value);

        let result = string_value_seed.serialize(&mut serializer);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unrecoverable parsing error: Serialized string (BOM + content + null terminator) length 15 exceeds fixed length 9");
    }

    #[test]
    fn test_dynamic_length_utf16_be_string_four_byte_len_serialize_succeeds() {
        let text = String::from("UTF16BE"); // 7 chars

        // Serializer uses BigEndian for length field as per SOMEIP_CONFIG
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::FourBytes, // Key: 4-byte length field
            bom: StringType::UTF_16_BE,
            fixed_length: None,
        };
        let proto_field_value = ReflectValueRef::String(&text);
        let string_value_seed = StringValueSeed::new(&string_type_definition, proto_field_value);

        string_value_seed.serialize(&mut serializer).expect("Serialization must succeed");

        let mut expected_bytes = Vec::new();
        // Calculate string data length: BOM (2) + content (7 chars * 2 bytes/char) + NUL (2 bytes)
        let string_data_len: u32 = (BOM_UTF16_BE.len() + text.len() * 2 + 2) as u32;

        // Length field (4 bytes, BigEndian)
        expected_bytes.extend_from_slice(&string_data_len.to_be_bytes());

        // String data
        expected_bytes.extend_from_slice(BOM_UTF16_BE);
        for char_utf16 in text.encode_utf16() {
            expected_bytes.extend_from_slice(&char_utf16.to_be_bytes());
        }
        expected_bytes.extend_from_slice(&0u16.to_be_bytes()); // Null terminator

        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }

    #[test]
    fn test_dynamic_length_utf16_le_string_four_byte_len_serialize_succeeds() {
        let text = String::from("UTF16LE"); // 7 chars

        // Serializer uses BigEndian for length field as per SOMEIP_CONFIG
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(SOMEIP_CONFIG);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::FourBytes, // Key: 4-byte length field
            bom: StringType::UTF_16_LE,
            fixed_length: None,
        };
        let proto_field_value = ReflectValueRef::String(&text);
        let string_value_seed = StringValueSeed::new(&string_type_definition, proto_field_value);

        string_value_seed.serialize(&mut serializer).expect("Serialization must succeed");

        let mut expected_bytes = Vec::new();
        // Calculate string data length: BOM (2) + content (7 chars * 2 bytes/char) + NUL (2 bytes)
        let string_data_len: u32 = (BOM_UTF16_LE.len() + text.len() * 2 + 2) as u32;

        // Length field (4 bytes, BigEndian because serializer is BE)
        expected_bytes.extend_from_slice(&string_data_len.to_be_bytes());

        // String data
        expected_bytes.extend_from_slice(BOM_UTF16_LE);
        for char_utf16 in text.encode_utf16() {
            expected_bytes.extend_from_slice(&char_utf16.to_le_bytes());
        }
        expected_bytes.extend_from_slice(&0u16.to_le_bytes()); // Null terminator

        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }
}
