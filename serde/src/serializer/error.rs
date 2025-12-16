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

use thiserror::Error;

/// Errors in the SOME/IP serialization module
#[derive(Error, Debug, PartialEq)]
pub enum SomeIpSerializeError {
    /// Failure to serialize a primitive value
    #[error("Failed to serialize primitive value")]
    Primitive,
    /// Failure to serialize a string value
    #[error("Failed to serialize string value")]
    String,
    /// Failure to serialize an array value
    #[error("Failed to serialize array value")]
    Array,
    /// Failure to serialize a struct value
    #[error("Failed to serialize struct value")]
    Struct,
    /// Unrecoverable parsing error
    #[error("Unrecoverable parsing error: {0}")]
    Failure(String),
    /// Out of range
    #[error("Out of range")]
    OutOfRange,
    /// Out of range
    #[error("Missing length field size for dynamic size value")]
    MissingLengthFieldSize,
}

impl serde::ser::Error for SomeIpSerializeError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        SomeIpSerializeError::Failure(msg.to_string())
    }
}
