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

//! Serde config for SOME/IP client

use serde::de::{self, Deserialize, Deserializer, Visitor};
use std::fmt;

/// This module contains struct definitions for the configuration to be used when
/// serializing and deserializing SOME/IP messages. These definitions are made available to
/// C++ code via 'cxx'. A side-effect of this 'Rust-to-C++' bridging is that the enums are
/// transformed into structs with a single 'u8' field called 'repr', with each enum variant
/// becoming a constant (See https://cxx.rs/shared.html#the-generated-data-structures for a more detailed explanation).
/// Practically, the syntax changes mostly affect pattern matching in Rust, with an extra
/// branch needed to account for unspecified values of the 'repr' field.
#[cxx::bridge]
pub mod ffi {
    /// Byte storage order used in SOME/IP serialization/deserialization
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum Endianness {
        /// Big Endian - most significant byte in lowest memory address
        BigEndian,
        /// Little Endian - most significant byte in highest memore address
        LittleEndian,
    }

    /// Size of the used length field in SOME/IP complex type serialization
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum LenFieldSize {
        /// Zero bytes (No length field)
        ZeroBytes,
        /// One Byte
        OneByte,
        /// Two Bytes
        TwoBytes,
        /// Four Bytes
        FourBytes,
    }

    /// Type of String used in SOME/IP serialization
    #[allow(non_camel_case_types)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum StringType {
        /// UTF 8
        UTF_8,
        /// UTF 16 Big Endian
        UTF_16_BE,
        /// UTF 16 Little Endian
        UTF_16_LE,
    }

    /// SOME/IP configuration for serialization/deserialization
    #[derive(Copy, Deserialize, Clone, Debug, PartialEq)]
    pub struct SomeIpConfig {
        /// Endianness
        pub endianness: Endianness,
        /// String Type
        pub string_type: StringType,
        /// Size of length field
        pub len_field_size: LenFieldSize,
    }
}

/// Implement custom deserialization for Endianness to be able to deserialize from constant names
impl<'de> Deserialize<'de> for self::ffi::Endianness {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EndiannessVisitor;

        impl Visitor<'_> for EndiannessVisitor {
            type Value = self::ffi::Endianness;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("Either BigEndian or LittleEndian")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                const VARIANTS: &[&str] = &["BigEndian", "LittleEndian"];
                match v {
                    "BigEndian" => Ok(self::ffi::Endianness::BigEndian),
                    "LittleEndian" => Ok(self::ffi::Endianness::LittleEndian),
                    var => Err(de::Error::unknown_variant(var, VARIANTS)),
                }
            }
        }
        deserializer.deserialize_str(EndiannessVisitor)
    }
}

/// Implement custom deserialization for LenFieldSize to be able to deserialize from constant names
impl<'de> Deserialize<'de> for self::ffi::LenFieldSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LenFieldSizeVisitor;

        impl Visitor<'_> for LenFieldSizeVisitor {
            type Value = self::ffi::LenFieldSize;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("One of ZeroBytes, OneByte, TwoBytes, FourBytes")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                const VARIANTS: &[&str] = &["ZeroBytes", "OneByte", "TwoBytes", "FourBytes"];
                match v {
                    "ZeroBytes" => Ok(self::ffi::LenFieldSize::ZeroBytes),
                    "OneByte" => Ok(self::ffi::LenFieldSize::OneByte),
                    "TwoBytes" => Ok(self::ffi::LenFieldSize::TwoBytes),
                    "FourBytes" => Ok(self::ffi::LenFieldSize::FourBytes),
                    var => Err(de::Error::unknown_variant(var, VARIANTS)),
                }
            }
        }
        deserializer.deserialize_str(LenFieldSizeVisitor)
    }
}

/// Implement custom deserialization for StringType to be able to deserialize from constant names
impl<'de> Deserialize<'de> for self::ffi::StringType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringTypeVisitor;

        impl Visitor<'_> for StringTypeVisitor {
            type Value = self::ffi::StringType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("One of UTF_8, UTF_16_BE, UTF_16_LE")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                const VARIANTS: &[&str] = &["UTF_8", "UTF_16_BE", "UTF_16_LE"];
                match v {
                    "UTF_8" => Ok(self::ffi::StringType::UTF_8),
                    "UTF_16_BE" => Ok(self::ffi::StringType::UTF_16_BE),
                    "UTF_16_LE" => Ok(self::ffi::StringType::UTF_16_LE),
                    var => Err(de::Error::unknown_variant(var, VARIANTS)),
                }
            }
        }
        deserializer.deserialize_str(StringTypeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};

    #[test]
    fn test_deserialize_endianness() {
        let be: Endianness = serde_json::from_str(r#""BigEndian""#).unwrap();
        assert_eq!(be, Endianness::BigEndian);

        let le: Endianness = serde_json::from_str(r#""LittleEndian""#).unwrap();
        assert_eq!(le, Endianness::LittleEndian);

        let invalid_json = r#""MiddleEndian""#;
        let result: Result<Endianness, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("unknown variant `MiddleEndian`"));
    }

    #[test]
    fn test_deserialize_len_field_size() {
        let zero: LenFieldSize = serde_json::from_str(r#""ZeroBytes""#).unwrap();
        assert_eq!(zero, LenFieldSize::ZeroBytes);

        let one: LenFieldSize = serde_json::from_str(r#""OneByte""#).unwrap();
        assert_eq!(one, LenFieldSize::OneByte);

        let two: LenFieldSize = serde_json::from_str(r#""TwoBytes""#).unwrap();
        assert_eq!(two, LenFieldSize::TwoBytes);

        let four: LenFieldSize = serde_json::from_str(r#""FourBytes""#).unwrap();
        assert_eq!(four, LenFieldSize::FourBytes);

        let invalid_json = r#""EightBytes""#;
        let result: Result<LenFieldSize, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("unknown variant `EightBytes`"));
    }

    #[test]
    fn test_deserialize_string_type() {
        let utf8: StringType = serde_json::from_str(r#""UTF_8""#).unwrap();
        assert_eq!(utf8, StringType::UTF_8);

        let utf16be: StringType = serde_json::from_str(r#""UTF_16_BE""#).unwrap();
        assert_eq!(utf16be, StringType::UTF_16_BE);

        let utf16le: StringType = serde_json::from_str(r#""UTF_16_LE""#).unwrap();
        assert_eq!(utf16le, StringType::UTF_16_LE);

        let invalid_json = r#""ASCII""#;
        let result: Result<StringType, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("unknown variant `ASCII`"));
    }

    #[test]
    fn test_deserialize_some_ip_config_ok() {
        let json_config = r#"
        {
            "endianness": "LittleEndian",
            "string_type": "UTF_8",
            "len_field_size": "TwoBytes"
        }
        "#;
        let config: SomeIpConfig = serde_json::from_str(json_config).unwrap();
        assert_eq!(
            config,
            SomeIpConfig {
                endianness: Endianness::LittleEndian,
                string_type: StringType::UTF_8,
                len_field_size: LenFieldSize::TwoBytes,
            }
        );
    }

    #[test]
    fn test_deserialize_some_ip_config_invalid_endianness() {
        let json_config = r#"
        {
            "endianness": "Invalid",
            "string_type": "UTF_8",
            "len_field_size": "TwoBytes"
        }
        "#;
        let result: Result<SomeIpConfig, _> = serde_json::from_str(json_config);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_some_ip_config_invalid_string_type() {
        let json_config = r#"
        {
            "endianness": "LittleEndian",
            "string_type": "Invalid",
            "len_field_size": "TwoBytes"
        }
        "#;
        let result: Result<SomeIpConfig, _> = serde_json::from_str(json_config);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_some_ip_config_invalid_len_field_size() {
        let json_config = r#"
        {
            "endianness": "LittleEndian",
            "string_type": "UTF_8",
            "len_field_size": "Invalid"
        }
        "#;
        let result: Result<SomeIpConfig, _> = serde_json::from_str(json_config);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_some_ip_config_missing_field() {
        let json_config = r#"
        {
            "endianness": "LittleEndian",
            "string_type": "UTF_8"
        }
        "#;
        let result: Result<SomeIpConfig, _> = serde_json::from_str(json_config);
        assert!(result.is_err());
        // Check that the error is about a missing field. This part is generated by serde derive.
        assert!(result.unwrap_err().to_string().contains("missing field `len_field_size`"));
    }
}
