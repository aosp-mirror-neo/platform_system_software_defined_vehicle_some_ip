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

//! Generator implementation for SOME/IP to Proto Translations for Known Types that Generates RRust Code

pub(crate) static INSERT_TYPE_PLACEHOLDER: &str = "<INSERT_TYPE>";
pub(crate) static PLACEHOLDER_FOR_I8: &str = "i8()?.into()";
pub(crate) static PLACEHOLDER_FOR_I16: &str = "i16()?.into()";
pub(crate) static PLACEHOLDER_FOR_I32: &str = "i32()?";
pub(crate) static PLACEHOLDER_FOR_I64: &str =
    "i64()?.try_into().map_err(|_| Err(SdvStatus::new(SdvStatusCode::OutOfRange)))?";
pub(crate) static PLACEHOLDER_FOR_U8: &str = "u8()?.into()";
pub(crate) static PLACEHOLDER_FOR_U16: &str = "u16()?.into()";
pub(crate) static PLACEHOLDER_FOR_U32: &str =
    "u32()?.try_into().map_err(|_| Err(SdvStatus::new(SdvStatusCode::OutOfRange)))?";
pub(crate) static PLACEHOLDER_FOR_U64: &str =
    "u64()?.try_into().map_err(|_| Err(SdvStatus::new(SdvStatusCode::OutOfRange)))?";
pub(crate) static INSTRUCTION_BOOLEAN_TYPE_BOOL: &str = "deserializer.deserialize_bool()?";
pub(crate) static INSTRUCTION_I8: &str = "deserializer.deserialize_i8()?.into()";
pub(crate) static INSTRUCTION_I16: &str = "deserializer.deserialize_i16()?.into()";
pub(crate) static INSTRUCTION_I32_TYPE_INT32: &str = "deserializer.deserialize_i32()?";
pub(crate) static INSTRUCTION_I32: &str = "deserializer.deserialize_i32()?.into()";
pub(crate) static INSTRUCTION_I64_TYPE_INT64: &str = "deserializer.deserialize_i64()?";
pub(crate) static INSTRUCTION_I64: &str = "deserializer.deserialize_i64()?.into()";
pub(crate) static INSTRUCTION_U8_TYPE_BYTES: &str = "deserializer.deserialize_u8()?";
pub(crate) static INSTRUCTION_U8: &str = "deserializer.deserialize_u8()?.into()";
pub(crate) static INSTRUCTION_U16: &str = "deserializer.deserialize_u16()?.into()";
pub(crate) static INSTRUCTION_U32_TYPE_UINT32: &str = "deserializer.deserialize_u32()?";
pub(crate) static INSTRUCTION_U32: &str = "deserializer.deserialize_u32()?.into()";
pub(crate) static INSTRUCTION_U64_TYPE_UINT64: &str = "deserializer.deserialize_u64()?";
pub(crate) static INSTRUCTION_U64: &str = "deserializer.deserialize_u64()?.into()";
pub(crate) static INSTRUCTION_F32_FLOAT: &str = "deserializer.deserialize_f32()?";
pub(crate) static INSTRUCTION_F32_DOUBLE: &str = "deserializer.deserialize_f32()?.into()";
pub(crate) static INSTRUCTION_F64_FLOAT: &str = "deserializer.deserialize_f64()?.into()";
pub(crate) static INSTRUCTION_F64_DOUBLE: &str = "deserializer.deserialize_f64()?";
pub(crate) static GLOBAL_TPL: &str = include_str!("templates/rust/global.tpl");
pub(crate) static DESERIALIZE_ENUM_TPL: &str = include_str!("templates/rust/deserialize_enum.tpl");
pub(crate) static DESERIALIZE_MESSAGE_TPL: &str =
    include_str!("templates/rust/deserialize_message.tpl");
pub(crate) static DESERIALIZE_STRING_TPL: &str =
    include_str!("templates/rust/deserialize_string.tpl");
pub(crate) static DESERIALIZE_ARRAY_TPL: &str =
    include_str!("templates/rust/deserialize_array.tpl");

pub(crate) fn deserialize_primitive_let_instruction(field_name: &str, rhs: &str) -> String {
    "let ".to_owned() + field_name + " = " + rhs + ";"
}

pub(crate) fn deserialize_substruct_let_instruction(field_name: &str, type_name: &str) -> String {
    "let ".to_owned()
        + field_name
        + " = Some("
        + deserialize_substruct_instruction(type_name).as_str()
        + ").into();"
}

pub(crate) fn deserialize_substruct_instruction(type_name: &str) -> String {
    "deserialize".to_owned()
        + type_name.replace(".", "_").to_lowercase().as_str()
        + "(deserializer)?"
}

pub(crate) fn deserialize_enum_instruction(type_name: &str) -> String {
    "deserialize_".to_owned()
        + type_name.replace(".", "_").to_lowercase().as_str()
        + "(deserializer)?.into()"
}
