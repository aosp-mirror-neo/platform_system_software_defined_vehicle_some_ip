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

use serde::de::{DeserializeSeed, Error, Visitor};
use serde::Deserializer;

use someip_config::serde_config::ffi::LenFieldSize;

pub(crate) struct LengthFieldDeserializeSeed<'lt> {
    length_field_size: &'lt LenFieldSize,
}

impl<'lt> LengthFieldDeserializeSeed<'lt> {
    pub(crate) fn new(length_field_size: &'lt LenFieldSize) -> Self {
        Self { length_field_size }
    }
}

impl<'de> DeserializeSeed<'de> for LengthFieldDeserializeSeed<'_> {
    type Value = usize;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        pub(crate) struct LengthFieldVisitor;

        impl Visitor<'_> for LengthFieldVisitor {
            type Value = usize;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP length field")
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v.into())
            }

            fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v.into())
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
            where
                E: Error,
            {
                usize::try_from(v)
                    .map_err(|_| E::custom(format!("Max allowed value is {}", usize::MAX)))
            }
        }

        match *self.length_field_size {
            LenFieldSize::ZeroBytes => {
                Err(D::Error::custom("No length field defined for dynamic size object"))
            }
            LenFieldSize::OneByte => deserializer.deserialize_u8(LengthFieldVisitor),
            LenFieldSize::TwoBytes => deserializer.deserialize_u16(LengthFieldVisitor),
            LenFieldSize::FourBytes => deserializer.deserialize_u32(LengthFieldVisitor),
            _ => Err(D::Error::custom("Unknown length field size")),
        }
    }
}

pub(crate) struct PaddingBytesDeserializeSeed {
    num_bytes: usize,
}

impl PaddingBytesDeserializeSeed {
    pub(crate) fn new(num_bytes: usize) -> Self {
        Self { num_bytes }
    }
}

impl<'de> DeserializeSeed<'de> for PaddingBytesDeserializeSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        pub(crate) struct PaddingBytesVisitor {
            num_bytes: usize,
        }

        impl PaddingBytesVisitor {
            pub(crate) fn new(num_bytes: usize) -> Self {
                Self { num_bytes }
            }
        }

        impl Visitor<'_> for PaddingBytesVisitor {
            type Value = ();

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP padding bytes")
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if self.num_bytes != value.len() {
                    return Err(E::custom("Internal error"));
                }
                Ok(())
            }
        }

        deserializer.deserialize_tuple(self.num_bytes, PaddingBytesVisitor::new(self.num_bytes))
    }
}

#[cfg(test)]
mod tests {
    use byteorder::LittleEndian;
    use serde::de::DeserializeSeed;
    use someip_config::serde_config::ffi::LenFieldSize;

    use crate::parser::common::LengthFieldDeserializeSeed;
    use crate::parser::common::PaddingBytesDeserializeSeed;
    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::error::SomeIpParseError;

    #[test]
    fn test_one_byte_len_field_deserialize_succeeds() {
        let value: u8 = 43;
        let data = vec![value];
        let len_field_size = LenFieldSize::OneByte;
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);
        let len_field_deserialize_seed = LengthFieldDeserializeSeed::new(&len_field_size);

        let result = len_field_deserialize_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");

        let expected: usize = value.into();

        assert_eq!(result, expected);
    }

    #[test]
    fn test_two_bytes_len_field_deserialize_succeeds() {
        let value: u16 = 89;
        let data = value.to_le_bytes();
        let len_field_size = LenFieldSize::TwoBytes;
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);
        let len_field_deserialize_seed = LengthFieldDeserializeSeed::new(&len_field_size);

        let result = len_field_deserialize_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");

        let expected: usize = value.into();

        assert_eq!(result, expected);
    }

    #[test]
    fn test_four_bytes_len_field_deserialize_succeeds() {
        let value: u32 = 83129;
        let data = value.to_le_bytes();
        let len_field_size = LenFieldSize::FourBytes;
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);
        let len_field_deserialize_seed = LengthFieldDeserializeSeed::new(&len_field_size);

        let result = len_field_deserialize_seed
            .deserialize(&mut deserializer)
            .expect("Deserialization must succeed");

        let expected: usize = value.try_into().expect("Value must fit into usize");

        assert_eq!(result, expected);
    }

    #[test]
    fn test_zero_bytes_len_field_deserialize_fails() {
        let value: u8 = 5;
        let data = value.to_le_bytes();
        let len_field_size = LenFieldSize::ZeroBytes;
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&data);
        let len_field_deserialize_seed = LengthFieldDeserializeSeed::new(&len_field_size);

        let result = len_field_deserialize_seed.deserialize(&mut deserializer);

        assert!(result.is_err());
    }

    #[test]
    fn test_padding_bytes_deserialize_succeeds() {
        let padding_data = vec![0x00, 0x00, 0x00];
        let num_padding_bytes = padding_data.len();
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&padding_data);
        let padding_seed = PaddingBytesDeserializeSeed::new(num_padding_bytes);

        let result = padding_seed.deserialize(&mut deserializer);
        assert!(result.is_ok(), "Deserialization failed: {:?}", result.err());
        assert_eq!(
            deserializer.num_read_bytes(),
            num_padding_bytes,
            "Deserializer did not consume the correct number of bytes"
        );
    }

    #[test]
    fn test_padding_bytes_deserialize_zero_bytes_succeeds() {
        let padding_data = vec![]; // No data
        let num_padding_bytes = 0;
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&padding_data);
        let padding_seed = PaddingBytesDeserializeSeed::new(num_padding_bytes);

        let result = padding_seed.deserialize(&mut deserializer);
        assert!(result.is_ok(), "Deserialization failed for zero bytes: {:?}", result.err());
        assert_eq!(
            deserializer.num_read_bytes(),
            0,
            "Deserializer consumed bytes when zero were expected"
        );
    }

    #[test]
    fn test_padding_bytes_deserialize_insufficient_data_fails() {
        let padding_data = vec![0x00, 0x00]; // Only 2 bytes available
        let num_padding_bytes_to_read = 3; // Expecting 3 bytes
        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&padding_data);
        let padding_seed = PaddingBytesDeserializeSeed::new(num_padding_bytes_to_read);

        let result = padding_seed.deserialize(&mut deserializer);
        assert!(result.is_err(), "Deserialization should have failed due to insufficient data");

        // The error should originate from the attempt to read more bytes than available
        // in `SomeIpSerdeDeserializer::deserialize_tuple`
        match result.unwrap_err() {
            SomeIpParseError::Failure(msg) => {
                assert!(msg.contains("Failed to read 3 bytes"));
            }
            _ => panic!("Unexpected error type"),
        }
    }
}
