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

//! Contains structures used to define SOME/IP types metadata that is used in reflection

use someip_config::serde_config::ffi::{LenFieldSize, StringType};
use std::fmt;

/// Metadata for a type to be (de)serialized using the SOME/IP protocol
/// Possible types of a SOME/IP message
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum SomeIpMessageTypeDefinition {
    /// Primitive data types
    PRIMITIVE(SomeIpPrimitiveTypeDefinition),
    /// String data type
    STRING(SomeIpStringTypeDefinition),
    /// Array data type
    ARRAY(SomeIpArrayTypeDefinition),
    /// Struct/Composite data type
    STRUCT(SomeIpStructTypeDefinition),
}

/// SOME/IP primitive types
#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
pub enum SomeIpPrimitiveTypeDefinition {
    /// SOME/IP boolean data definition
    BOOLEAN,
    /// SOME/IP U8 data definition
    U8,
    /// SOME/IP U16 data definition
    U16,
    /// SOME/IP U32 data definition
    U32,
    /// SOME/IP U64 data definition
    U64,
    /// SOME/IP I8 data definition
    I8,
    /// SOME/IP I16 data definition
    I16,
    /// SOME/IP I32 data definition
    I32,
    /// SOME/IP I64 data definition
    I64,
    /// SOME/IP F32 data definition
    F32,
    /// SOME/IP F64 data definition
    F64,
}

impl fmt::Display for SomeIpPrimitiveTypeDefinition {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SomeIpPrimitiveTypeDefinition::BOOLEAN => fmt.write_str("BOOLEAN")?,
            SomeIpPrimitiveTypeDefinition::U8 => fmt.write_str("U8")?,
            SomeIpPrimitiveTypeDefinition::U16 => fmt.write_str("U16")?,
            SomeIpPrimitiveTypeDefinition::U32 => fmt.write_str("U32")?,
            SomeIpPrimitiveTypeDefinition::U64 => fmt.write_str("U64")?,
            SomeIpPrimitiveTypeDefinition::I8 => fmt.write_str("I8")?,
            SomeIpPrimitiveTypeDefinition::I16 => fmt.write_str("I16")?,
            SomeIpPrimitiveTypeDefinition::I32 => fmt.write_str("I32")?,
            SomeIpPrimitiveTypeDefinition::I64 => fmt.write_str("I64")?,
            SomeIpPrimitiveTypeDefinition::F32 => fmt.write_str("F32")?,
            SomeIpPrimitiveTypeDefinition::F64 => fmt.write_str("F64")?,
        };
        Ok(())
    }
}

/// Metadata for SOME/IP string
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct SomeIpStringTypeDefinition {
    /// Ignored if this is a fixed length string. For dynamic length strings, this represents the length of
    /// the 'length' field in bytes. If set to 0, a default length size of four bytes is used
    pub length_field_size: LenFieldSize,
    /// Represents the encoding used for this string
    pub bom: StringType,
    /// If present, represents length of string in bytes. If absent, the string's length is present at the start
    /// of the string
    pub fixed_length: Option<usize>,
}

/// Metadata for SOME/IP array type
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SomeIpArrayTypeDefinition {
    /// If present, represents the number of elements in the array. If absent, the array's length is present at the
    /// start of the array
    pub fixed_length: Option<usize>,
    /// Ignored if this is a fixed length array. For dynamic length arrays, this represents the size of
    /// the length field used to store the number of elements in the array. If set to 0, the array MUST be a fixed length array
    pub length_field_size: LenFieldSize,
    /// Type of the array elements
    pub array_type: Box<SomeIpMessageTypeDefinition>,
}

/// Metadata for SOME/IP struct type
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SomeIpStructTypeDefinition {
    /// Struct name
    pub name: String,
    /// Size of the length field used by the struct, used to store the number of bytes the serialized struct uses.
    /// This affects deserialization of the struct in that it checks that the deserialized struct's length in bytes
    /// is less than the specified number of bytes
    pub length_field_size: LenFieldSize,
    /// Array of type definitions for the struct's fields
    pub fields: Vec<SomeIpStructFieldTypeDefinition>,
}

/// Metadata for a SOME/IP struct field type
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SomeIpStructFieldTypeDefinition {
    /// An optional data tag used to identify which struct field is read next. If absent, the struct's fields are serialized
    /// in the order in which they appear in the struct definition. TODO: Handle data tags in SOME/IP structs
    pub data_tag: Option<SomeIpFieldTagDefinition>,
    /// Struct field name
    pub field_name: String,
    /// Struct field type definition
    pub field_type: SomeIpMessageTypeDefinition,
}

/// Metadata for SOME/IP field tag
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SomeIpFieldTagDefinition {
    wire_type: u8,
    data_id: u16,
}

/// Data for SOME/IP message
#[derive(PartialEq, Debug, Clone)]
pub enum SomeIpMessageData {
    /// Primitive data
    PRIMITIVE(SomeIpPrimitiveType),
    /// String data
    STRING(String),
    /// Array data
    ARRAY(SomeIpArrayType),
    /// Struct/Composite data
    STRUCT(SomeIpStructType),
}

/// Data for SOME/IP primitive type
#[derive(PartialEq, Clone, Debug)]
pub enum SomeIpPrimitiveType {
    /// SOME/IP boolean data
    BOOLEAN(bool),
    /// SOME/IP U8 data
    U8(u8),
    /// SOME/IP U16 data
    U16(u16),
    /// SOME/IP U32 data
    U32(u32),
    /// SOME/IP U64 data
    U64(u64),
    /// SOME/IP I8 data
    I8(i8),
    /// SOME/IP I16 data
    I16(i16),
    /// SOME/IP I32 data
    I32(i32),
    /// SOME/IP I64 data
    I64(i64),
    /// SOME/IP F32 data
    F32(f32),
    /// SOME/IP F64 data
    F64(f64),
}

/// Data for SOME/IP array message
#[derive(PartialEq, Debug, Clone)]
pub struct SomeIpArrayType {
    /// array element data
    pub array_data: Vec<SomeIpMessageData>,
}

/// Metadata for SOME/IP struct type
#[derive(PartialEq, Debug, Clone)]
pub struct SomeIpStructType {
    /// Struct fields
    pub fields: Vec<SomeIpStructFieldType>,
}

/// Data for SOME/IP struct field
#[derive(PartialEq, Debug, Clone)]
pub struct SomeIpStructFieldType {
    /// Struct field name
    pub field_name: String,
    /// Struct field data
    pub field_data: SomeIpMessageData,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_some_ip_primitive_type_definition() {
        assert_eq!(SomeIpPrimitiveTypeDefinition::BOOLEAN.to_string(), "BOOLEAN");
        assert_eq!(SomeIpPrimitiveTypeDefinition::U8.to_string(), "U8");
        assert_eq!(SomeIpPrimitiveTypeDefinition::U16.to_string(), "U16");
        assert_eq!(SomeIpPrimitiveTypeDefinition::U32.to_string(), "U32");
        assert_eq!(SomeIpPrimitiveTypeDefinition::U64.to_string(), "U64");
        assert_eq!(SomeIpPrimitiveTypeDefinition::I8.to_string(), "I8");
        assert_eq!(SomeIpPrimitiveTypeDefinition::I16.to_string(), "I16");
        assert_eq!(SomeIpPrimitiveTypeDefinition::I32.to_string(), "I32");
        assert_eq!(SomeIpPrimitiveTypeDefinition::I64.to_string(), "I64");
        assert_eq!(SomeIpPrimitiveTypeDefinition::F32.to_string(), "F32");
        assert_eq!(SomeIpPrimitiveTypeDefinition::F64.to_string(), "F64");
    }
}
