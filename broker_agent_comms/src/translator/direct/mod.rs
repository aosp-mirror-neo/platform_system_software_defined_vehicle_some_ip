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

mod incoming;
mod valid_mappings;

pub(crate) use incoming::group_metadata;
pub(crate) use incoming::metadata;
pub(crate) use incoming::DirectTranslator;
pub(crate) use incoming::ProtoFormatData;

use crate::translator::direct::incoming::ISomeIpReader;
use anyhow::bail;
use someip_config::serde_config::ffi::LenFieldSize;
use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

// Defines size properties for SOME/IP arrays and strings
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum SomeIpSizeType {
    // A fixed-size type in bytes
    FIXED(usize),
    // A variable-size type; indicates the byte length of the size field
    VARIABLE(LenFieldSize),
}

impl SomeIpSizeType {
    /// This function is intended to be used when creating a `SomeIpSizeType` from a mapping
    /// configuration. For direct construction, prefer using `SomeIpSizeType::FIXED` or
    /// `SomeIpSizeType::VARIABLE` directly.
    pub(crate) fn new(
        fixed_size: Option<usize>,
        len_field_size: LenFieldSize,
    ) -> anyhow::Result<Self> {
        if let Some(size) = fixed_size {
            Ok(SomeIpSizeType::FIXED(size))
        } else if len_field_size == LenFieldSize::OneByte
            || len_field_size == LenFieldSize::TwoBytes
            || len_field_size == LenFieldSize::FourBytes
        {
            Ok(SomeIpSizeType::VARIABLE(len_field_size))
        } else {
            bail!("Invalid length field size for variable size type")
        }
    }

    pub(crate) fn get_size_from_someip_format<'input_lt, R: ISomeIpReader<'input_lt>>(
        &self,
        someip_reader: &mut R,
    ) -> anyhow::Result<usize> {
        match self {
            SomeIpSizeType::FIXED(size) => Ok(*size),
            SomeIpSizeType::VARIABLE(len_field_size) => {
                let size = match *len_field_size {
                    LenFieldSize::OneByte => someip_reader.read_u8()? as usize,
                    LenFieldSize::TwoBytes => someip_reader.read_u16()? as usize,
                    LenFieldSize::FourBytes => someip_reader.read_u32()? as usize,
                    _ => {
                        // logic in metadata builder ensures we never construct an invalid
                        // SomeIpSizeType
                        unreachable!()
                    }
                };
                Ok(size)
            }
        }
    }
}

pub(crate) trait SomeIpPrimitiveSize {
    fn get_size(&self) -> usize;
}

impl SomeIpPrimitiveSize for SomeIpPrimitiveTypeDefinition {
    fn get_size(&self) -> usize {
        match self {
            SomeIpPrimitiveTypeDefinition::BOOLEAN => 1,
            SomeIpPrimitiveTypeDefinition::U8 => 1,
            SomeIpPrimitiveTypeDefinition::U16 => 2,
            SomeIpPrimitiveTypeDefinition::U32 => 4,
            SomeIpPrimitiveTypeDefinition::U64 => 8,
            SomeIpPrimitiveTypeDefinition::I8 => 1,
            SomeIpPrimitiveTypeDefinition::I16 => 2,
            SomeIpPrimitiveTypeDefinition::I32 => 4,
            SomeIpPrimitiveTypeDefinition::I64 => 8,
            SomeIpPrimitiveTypeDefinition::F32 => 4,
            SomeIpPrimitiveTypeDefinition::F64 => 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_fixed_size() {
        let size = 10;
        let result = SomeIpSizeType::new(Some(size), LenFieldSize::ZeroBytes).unwrap();
        assert_eq!(result, SomeIpSizeType::FIXED(size));
    }

    #[test]
    fn test_new_variable_one_byte() {
        let len_field_size = LenFieldSize::OneByte;
        let result = SomeIpSizeType::new(None, len_field_size).unwrap();
        assert_eq!(result, SomeIpSizeType::VARIABLE(len_field_size));
    }

    #[test]
    fn test_new_variable_two_bytes() {
        let len_field_size = LenFieldSize::TwoBytes;
        let result = SomeIpSizeType::new(None, len_field_size).unwrap();
        assert_eq!(result, SomeIpSizeType::VARIABLE(len_field_size));
    }

    #[test]
    fn test_new_variable_four_bytes() {
        let len_field_size = LenFieldSize::FourBytes;
        let result = SomeIpSizeType::new(None, len_field_size).unwrap();
        assert_eq!(result, SomeIpSizeType::VARIABLE(len_field_size));
    }

    #[test]
    fn test_new_invalid_variable_size() {
        let result = SomeIpSizeType::new(None, LenFieldSize::ZeroBytes);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid length field size for variable size type"
        );
    }

    #[test]
    fn test_new_fixed_size_ignores_len_field() {
        let size = 20;
        // Even with a valid variable len_field_size, fixed_size should take precedence.
        let result = SomeIpSizeType::new(Some(size), LenFieldSize::FourBytes).unwrap();
        assert_eq!(result, SomeIpSizeType::FIXED(size));
    }
}
