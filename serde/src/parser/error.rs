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

/// Errors in the SOME/IP deserialization module
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SomeIpParseError {
    /// Failure to parse a primitive value
    #[error("Failed to parse primitive value")]
    Primitive,
    /// Failure to parse a string value
    #[error("Failed to parse string value")]
    String,
    /// Failure to parse an array value
    #[error("Failed to parse array value")]
    Array,
    /// Failure to parse a struct value
    #[error("Failed to parse struct value")]
    Struct,
    /// Unrecoverable parsing error
    #[error("Unrecoverable parsing error: {0}")]
    Failure(String),
    /// Out of range
    #[error("Out of range")]
    OutOfRange,
}

impl serde::de::Error for SomeIpParseError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        SomeIpParseError::Failure(format!("{msg}"))
    }
}
