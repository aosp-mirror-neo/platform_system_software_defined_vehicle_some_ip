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

use serde::de::{DeserializeSeed, Error, SeqAccess, Visitor};
use serde::Deserializer;

use crate::parser::common::{LengthFieldDeserializeSeed, PaddingBytesDeserializeSeed};
use someip_serde_reflect::SomeIpStringTypeDefinition;

pub(crate) struct StringValueSeed<'lt> {
    someip_string_definition: &'lt SomeIpStringTypeDefinition,
}

impl<'lt> StringValueSeed<'lt> {
    pub(crate) fn new(someip_string_definition: &'lt SomeIpStringTypeDefinition) -> Self {
        Self { someip_string_definition }
    }
}

impl<'de> DeserializeSeed<'de> for StringValueSeed<'_> {
    type Value = String;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringBytesDeserializeSeed;

        impl<'de> DeserializeSeed<'de> for StringBytesDeserializeSeed {
            type Value = String;

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct StringBytesVisitor;

                impl Visitor<'_> for StringBytesVisitor {
                    type Value = String;

                    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(fmt, "SOME/IP string bytes")
                    }

                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        self.visit_string(v.to_string())
                    }

                    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(value)
                    }
                }
                deserializer.deserialize_string(StringBytesVisitor)
            }
        }

        struct StringValueVisitor<'lt> {
            someip_string_definition: &'lt SomeIpStringTypeDefinition,
        }

        impl<'de> Visitor<'de> for StringValueVisitor<'_> {
            type Value = String;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP string")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let string_size = {
                    if let Some(size) = self.someip_string_definition.fixed_length {
                        size
                    } else {
                        let optional_size =
                            seq.next_element_seed(LengthFieldDeserializeSeed::new(
                                &self.someip_string_definition.length_field_size,
                            ))?;
                        if let Some(size) = optional_size {
                            size
                        } else {
                            return Err(A::Error::custom(
                                "SOME/IP dynamic length string has no size",
                            ));
                        }
                    }
                };
                let remaining_bytes_start =
                    seq.size_hint().ok_or_else(|| A::Error::custom("Unknown string size"))?;
                if let Some(string_value) = seq.next_element_seed(StringBytesDeserializeSeed)? {
                    let remaining_bytes_end =
                        seq.size_hint().ok_or_else(|| A::Error::custom("Unknown string size"))?;
                    let string_size_in_bytes = remaining_bytes_start - remaining_bytes_end;

                    if string_size_in_bytes > string_size {
                        Err(A::Error::custom(format!(
                            "SOME/IP string exceeds expected size - expected {string_size}, actual {string_size_in_bytes}"
                        )))
                    } else {
                        let remaining_bytes = string_size - string_size_in_bytes;
                        if remaining_bytes > 0 {
                            // read remaining bytes
                            seq.next_element_seed(PaddingBytesDeserializeSeed::new(
                                remaining_bytes,
                            ))?;
                        }
                        Ok(string_value)
                    }
                } else {
                    Err(A::Error::custom("Error deserializing string"))
                }
            }
        }

        let string_field_visitor =
            StringValueVisitor { someip_string_definition: self.someip_string_definition };

        deserializer.deserialize_seq(string_field_visitor)
    }
}

#[cfg(test)]
mod tests {
    use byteorder::LittleEndian;
    use serde::de::DeserializeSeed;
    use someip_config::serde_config::ffi::{LenFieldSize, StringType};

    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::someip::string::StringValueSeed;

    use someip_serde_reflect::SomeIpStringTypeDefinition;

    const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];
    const BOM_UTF16_BE: &[u8] = &[0xFE, 0xFF];
    const BOM_UTF16_LE: &[u8] = &[0xFF, 0xFE];

    #[test]
    fn test_dynamic_length_string_deserialize_succeeds() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.push(3 + 5 + 1);
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result =
            string_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(result, text);
    }

    #[test]
    fn test_dynamic_length_string_utf16be_deserialize_succeeds() {
        let text = String::from("hello");

        let text_u16_rep: Vec<u16> = text.encode_utf16().collect();
        let mut text_bytes = Vec::new();
        for v in text_u16_rep {
            text_bytes.push((v >> 8) as u8);
            text_bytes.push((v & 0xFF) as u8);
        }

        let mut data = Vec::new();
        // 2 bytes for BOM length + 10 bytes for "hello" string bytes in UTF16 + 2 bytes null bytes
        data.push(2 + 10 + 2);
        data.extend_from_slice(BOM_UTF16_BE);
        data.extend_from_slice(&text_bytes);
        data.push(0x00);
        data.push(0x00);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_16_BE,
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result =
            string_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(result, text);
    }

    #[test]
    fn test_dynamic_length_string_utf16le_deserialize_succeeds() {
        let text = String::from("hello");

        let text_u16_rep: Vec<u16> = text.encode_utf16().collect();
        let mut text_bytes = Vec::new();
        for v in text_u16_rep {
            text_bytes.push((v & 0xFF) as u8);
            text_bytes.push((v >> 8) as u8);
        }

        let mut data = Vec::new();
        // 2 bytes for BOM length + 10 bytes for "hello" string bytes in UTF16 + 2 bytes null bytes
        data.push(2 + 10 + 2);
        data.extend_from_slice(BOM_UTF16_LE);
        data.extend_from_slice(&text_bytes);
        data.push(0x00);
        data.push(0x00);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_16_LE,
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result =
            string_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(result, text);
    }

    #[test]
    fn test_fixed_length_string_deserialize_succeeds() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: Some(3 + 5 + 1),
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result =
            string_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(result, text);
    }

    #[test]
    fn test_string_deserialize_data_too_short_for_bom() {
        let data = vec![BOM_UTF8[0]]; // Only 1 byte, less than min BOM length (2)

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::ZeroBytes, // Not used for fixed_length
            bom: StringType::UTF_8,
            fixed_length: Some(1), // String size is 1
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result = string_value_seed.deserialize(&mut deserializer);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Failed to parse string value");
    }

    #[test]
    fn test_string_deserialize_unknown_bom() {
        let unknown_bom_data = vec![0x01, 0x02, 0x03, b'h', b'e', b'l', b'l', b'o', 0x00];
        let mut data = Vec::new();
        data.push(unknown_bom_data.len() as u8); // Length field
        data.extend_from_slice(&unknown_bom_data);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        // Definition expects UTF_8
        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result = string_value_seed.deserialize(&mut deserializer);

        assert!(result.is_err());
        // Assuming the error message is corrected to use a fixed slice or avoid bom_bytes_expected
        assert_eq!(result.unwrap_err().to_string(), "Failed to parse string value");
    }

    #[test]
    fn test_string_deserialize_definition_mismatch_but_valid_bom_present() {
        let text = String::from("ok");
        let mut payload_data = Vec::new();
        payload_data.extend_from_slice(BOM_UTF16_LE); // Actual data is UTF-16LE
        payload_data.extend_from_slice(
            &text.encode_utf16().flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>(),
        );
        payload_data.extend_from_slice(&[0x00, 0x00]); // Null terminator

        let mut data = Vec::new();
        data.push(payload_data.len() as u8);
        data.extend_from_slice(&payload_data);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        // Definition expects UTF_8, but data contains a valid UTF-16LE BOM
        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8, // Definition mismatch
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        // According to the new logic, if a valid BOM is detected, it should be used.
        let result = string_value_seed.deserialize(&mut deserializer);
        assert!(
            result.is_ok(),
            "Expected successful parsing with detected BOM, got: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), text);
    }

    #[test]
    fn test_string_deserialize_with_no_length_fails() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::ZeroBytes,
            bom: StringType::UTF_8,
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result = string_value_seed.deserialize(&mut deserializer);

        assert!(result.is_err());
    }

    #[test]
    fn test_dynamic_string_deserialize_with_length_field_larger_than_actual_data_fails() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.push(30); // arbitrary large length
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result = string_value_seed.deserialize(&mut deserializer);
        // Expect error because the deserialize op will go out of bounds.
        // Payload is 1 (len field) + 3 (BOM) + 5 (hello) + 1 (NUL) = 10 bytes.
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to read 21 bytes"));
    }

    #[test]
    fn test_dynamic_string_deserialize_with_length_field_smaller_than_actual_data_fails() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.push(3); // arbitrary small length
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte,
            bom: StringType::UTF_8,
            fixed_length: None,
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result = string_value_seed.deserialize(&mut deserializer);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: SOME/IP string exceeds expected size - expected 3, actual 9"
        );
    }

    #[test]
    fn test_fixed_length_string_payload_longer_than_definition_fails() {
        // Payload for "hello world" (BOM + "hello world" + NUL = 3 + 11 + 1 = 15 bytes)
        let text_long = "hello world";
        let mut payload = Vec::new();
        payload.extend_from_slice(BOM_UTF8);
        payload.extend_from_slice(text_long.as_bytes());
        payload.push(0x00); // Null terminator
                            // payload is now 15 bytes long.

        // The deserializer will read fixed_length = 9 bytes from the payload.
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte, // Not used for fixed_length
            bom: StringType::UTF_8,
            fixed_length: Some(9), // Intended for "hello" (3 + 5 + 1), but payload is "hello world"
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result = string_value_seed.deserialize(&mut deserializer);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: SOME/IP string exceeds expected size - expected 9, actual 15"
        );
    }

    #[test]
    fn test_fixed_length_string_payload_shorter_than_definition_fails() {
        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        // Payload is 9 bytes long.
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::ZeroBytes,
            bom: StringType::UTF_8,
            fixed_length: Some(30),
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result = string_value_seed.deserialize(&mut deserializer);
        // deserialize_string will read the 9 bytes of the string, then attempt to read 21
        // bytes of padding (9 + 21 = 30 bytes) but none are available.
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unrecoverable parsing error: Failed to read 21 bytes: SdvStatusCode::OutOfRange(11)"
        );
    }

    #[test]
    fn test_fixed_length_string_payload_with_padding_succeeds() {
        let text_short = "hello";
        let mut payload = Vec::new();
        payload.extend_from_slice(BOM_UTF8);
        payload.extend_from_slice(text_short.as_bytes());
        payload.push(0x00); // Null terminator (total 9 bytes so far)

        let padding_len = 10;
        payload.extend_from_slice(&vec![0x00; padding_len]);

        // Total payload length = 9 (BOM + "hello" + NUL) + 10 (padding) = 19 bytes

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);

        let string_type_definition = SomeIpStringTypeDefinition {
            length_field_size: LenFieldSize::OneByte, // Not used for fixed_length
            bom: StringType::UTF_8,
            fixed_length: Some(9 + padding_len), // Fixed length includes padding
        };
        let string_value_seed = StringValueSeed::new(&string_type_definition);

        let result =
            string_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        assert_eq!(result, text_short);

        // Check that the deserializer consumed exactly fixed_length bytes
        // The SomeIpDeserializer (inner) index should be at the end of the fixed length.
        // The string_size passed to deserialize_tuple was 19.
        // The SomeIpSerdeDeserializer's deserialize_tuple reads exactly that many bytes.
        // So, the underlying SomeIpDeserializer's index should be 19.
        assert_eq!(deserializer.num_read_bytes(), 9 + padding_len);
    }
}
