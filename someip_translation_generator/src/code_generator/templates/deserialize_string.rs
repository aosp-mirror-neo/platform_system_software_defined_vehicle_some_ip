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

use serde_derive::Serialize;
use tinytemplate::TinyTemplate;

#[derive(Serialize)]
struct Context<'a> {
    field_name: &'a str,
    length_is_fixed: bool,
    length_is_one_byte: bool,
    length_is_two_bytes: bool,
    length_is_four_bytes: bool,
    length: usize,
}

impl<'a> Context<'a> {
    pub(self) fn from(field_name: &'a str, length_type: LengthType) -> Self {
        Context {
            field_name,
            length_is_fixed: matches!(length_type, LengthType::FixedLength(_)),
            length_is_one_byte: matches!(length_type, LengthType::OneByte),
            length_is_two_bytes: matches!(length_type, LengthType::TwoBytes),
            length_is_four_bytes: matches!(length_type, LengthType::FourBytes),
            length: match length_type {
                LengthType::FixedLength(len) => len,
                _ => 0,
            },
        }
    }
}

#[derive(PartialEq)]
pub(crate) enum LengthType {
    FixedLength(usize),
    OneByte,
    TwoBytes,
    FourBytes,
}

pub(crate) fn render(
    field_name: &str,
    length_type: LengthType,
    tt_engine: &TinyTemplate,
) -> anyhow::Result<String> {
    tt_engine.render("deserialize_string", &Context::from(field_name, length_type)).map_err(|e| {
        anyhow::Error::msg(format!("TinyTemplate failed to generate deserialize_string: {e:?}"))
    })
}
