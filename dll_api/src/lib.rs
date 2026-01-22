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
//! This crate provides helper methods for deserializing SOME/IP wire format

mod deserializer;

use byteorder::{BigEndian, LittleEndian};
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};

pub use sdv_comms_bindgen::ASDV_UnitType;
pub use someip_config::serde_config::ffi::Endianness;

pub use crate::deserializer::{ISomeIpDeserializer, SomeIpDeserializer};

/// Create a SOME/IP deserializer
pub fn create_someip_deserializer<'input_lt>(
    input: &'input_lt [u8],
    endianness: Endianness,
) -> SdvResult<Box<dyn ISomeIpDeserializer + 'input_lt>> {
    match endianness {
        Endianness::BigEndian => Ok(Box::new(SomeIpDeserializer::<BigEndian>::new(input))),
        Endianness::LittleEndian => Ok(Box::new(SomeIpDeserializer::<LittleEndian>::new(input))),
        _ => Err(SdvStatus::with_message(
            SdvStatusCode::InvalidArgument,
            "Unknown endianness argument",
        )),
    }
}
