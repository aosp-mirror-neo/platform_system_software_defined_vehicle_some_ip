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

use chrono::Datelike;
use serde_derive::Serialize;
use tinytemplate::TinyTemplate;

#[derive(Serialize)]
struct TopFunctionMatchLine<'a> {
    unit_type_full_name: &'a str,
    deserialize_function_name: &'a str,
}

#[derive(Serialize)]
struct Context<'a> {
    year: i32,
    header_use_lines: &'a str,
    top_function_match_lines: Vec<TopFunctionMatchLine<'a>>,
    deserialization_implementation_lines: &'a str,
}

impl<'a> Context<'a> {
    pub(self) fn from(
        header_use_lines: &'a str,
        top_function_match_lines: &'a [(String, String)],
        deserialization_implementation_lines: &'a str,
    ) -> Self {
        Context {
            year: get_current_year(),
            header_use_lines,
            top_function_match_lines: top_function_match_lines
                .iter()
                .map(|(unit_type_full_name, deserialize_function_name)| TopFunctionMatchLine {
                    unit_type_full_name,
                    deserialize_function_name,
                })
                .collect(),
            deserialization_implementation_lines,
        }
    }
}

pub(crate) fn render(
    header_use_lines: &str,
    top_function_match_lines: &[(String, String)],
    deserialization_implementation_lines: &str,
    tt_engine: &TinyTemplate,
) -> anyhow::Result<String> {
    tt_engine
        .render(
            "global",
            &Context::from(
                header_use_lines,
                top_function_match_lines,
                deserialization_implementation_lines,
            ),
        )
        .map_err(|e| anyhow::Error::msg(format!("TinyTemplate failed to generate global: {e:?}")))
}

fn get_current_year() -> i32 {
    chrono::Utc::now().year()
}
