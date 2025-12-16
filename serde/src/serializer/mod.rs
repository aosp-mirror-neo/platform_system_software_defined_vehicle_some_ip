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
//! Contains method for serializing protobuf messages into SOME/IP wire format

use byteorder::{BigEndian, LittleEndian};

use protobuf::reflect::MessageDescriptor;
use protobuf::MessageDyn;

use serde::Serialize;
use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig};

use complex::StructValueSeed;
pub use error::SomeIpSerializeError;
use serializer::SomeIpSerdeSerializer;
use someip_serde_reflect::SomeIpStructTypeDefinition;

mod array;
mod complex;
mod error;
mod primitive;
#[allow(clippy::module_inception)]
mod serializer;
mod string;

pub(crate) fn get_byte_size(len_field_size: &LenFieldSize) -> Result<usize, SomeIpSerializeError> {
    match *len_field_size {
        LenFieldSize::ZeroBytes => Err(SomeIpSerializeError::MissingLengthFieldSize),
        LenFieldSize::OneByte => Ok(1),
        LenFieldSize::TwoBytes => Ok(2),
        LenFieldSize::FourBytes => Ok(4),
        _ => Err(SomeIpSerializeError::MissingLengthFieldSize),
    }
}

/// Translate protobuf dyn message to SOME/IP wire format
#[allow(unused)]
#[tracing::instrument(skip_all)]
pub fn get_someip_bytes_from_message_dyn(
    config: SomeIpConfig,
    someip_struct_definition: &SomeIpStructTypeDefinition,
    proto_message_descriptor: &MessageDescriptor,
    message: &dyn MessageDyn,
) -> Result<Vec<u8>, SomeIpSerializeError> {
    match config.endianness {
        Endianness::BigEndian => {
            let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(config);
            let struct_value_seed =
                StructValueSeed::new(someip_struct_definition, proto_message_descriptor, message);
            struct_value_seed.serialize(&mut serializer)?;
            Ok(serializer.get_serialized_bytes())
        }
        Endianness::LittleEndian => {
            let mut serializer = SomeIpSerdeSerializer::<LittleEndian>::new(config);
            let struct_value_seed =
                StructValueSeed::new(someip_struct_definition, proto_message_descriptor, message);
            struct_value_seed.serialize(&mut serializer)?;
            Ok(serializer.get_serialized_bytes())
        }
        _ => Err(SomeIpSerializeError::Failure("Unknown endiannes".to_string())),
    }
}
