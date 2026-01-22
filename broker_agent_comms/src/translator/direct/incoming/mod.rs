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

mod array_metadata;
mod direct_translator;
pub(crate) mod group_metadata;
mod group_reader;
pub(crate) mod metadata;
mod primitive_metadata;
mod proto_format_data;
mod reader;
mod string_metadata;
mod struct_metadata;

pub(crate) use direct_translator::DirectTranslator;
pub(crate) use proto_format_data::ProtoFormatData;
pub(super) use reader::{ISomeIpReader, SomeIpReader};
