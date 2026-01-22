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

mod broker_translator;
mod descriptor_store;
mod direct;

use sdv_comms::sd::UnitType;
use someip_serde::parser::SomeIpParseError;
use someip_serde::serializer::SomeIpSerializeError;

pub(crate) use broker_translator::BrokerTranslator;
pub(crate) use broker_translator::IBrokerTranslator;
pub(crate) use broker_translator::TranslationMode;
pub(crate) use direct::DirectTranslator;
pub(crate) use direct::ProtoFormatData;

pub(crate) use descriptor_store::{RpcMode, SomeIpDescriptorId, SomeIpDescriptorStore};

#[cfg(test)]
pub(crate) use broker_translator::MockIBrokerTranslator;

/// Error type used when accessing dynamic messages
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum TranslationError {
    /// Unknown message
    #[error("Unknown message {0}")]
    UnknownMessage(String),
    /// Unknown unit type
    #[error("Unknown unit type {0:?}")]
    UnknownUnitType(UnitType),
    // Unknown method for unit type
    #[error("Unknown method {1} on unit type {0:?}")]
    UnknownUnitTypeMethod(UnitType, String),
    // SOME/IP Parsing error
    #[error("SOME/IP Parsing error {0:?}")]
    SomeIpParserError(#[from] SomeIpParseError),
    // SOME/IP Serialization error
    #[error("SOME/IP Serialization error {0:?}")]
    SomeIpSerializerError(#[from] SomeIpSerializeError),
    /// Unknown message field
    #[error("Unknown field name {0}")]
    UnknownField(String),
    /// Unknown topic
    #[error("Expected a SOME/IP struct type definition for topic {0}")]
    UnexpectedSomeIpType(String),
    /// Mismatched field type
    #[error("Expected type {0}, found type {1}")]
    MismatchedType(String, String),
    /// Internal Protobuf error
    #[error("Protobuf Error {0}")]
    ProtobufError(#[from] protobuf::Error),
    /// Out of range error
    #[error("Out of range error")]
    OutOfRange,
    /// VSIDL Config Provider error
    #[error("VsidlConfigProvider Error")]
    ConfigProviderError,
    /// Mutex lock failed because of panic in another thread
    #[error("Mutex is poisoned")]
    MutexPoisonedError,
}
