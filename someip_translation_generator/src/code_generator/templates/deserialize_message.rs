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
struct StructInitField<'a> {
    name: &'a str,
    is_optional: &'a bool,
}

#[derive(Serialize)]
struct Context<'a> {
    unit_type_name: &'a str,
    unit_type_name_lowercase: String,
    unit_type_name_stripped: &'a str,
    let_statements: &'a Vec<String>,
    struct_init_fields: Vec<StructInitField<'a>>,
}

impl<'a> Context<'a> {
    pub(self) fn from(
        unit_type_name: &'a str,
        unit_type_name_stripped: &'a str,
        let_statements: &'a Vec<String>,
        struct_init_fields: Vec<StructInitField<'a>>,
    ) -> Self {
        Context {
            unit_type_name,
            unit_type_name_lowercase: unit_type_name.replace(".", "_").to_lowercase(),
            unit_type_name_stripped,
            let_statements,
            struct_init_fields,
        }
    }
}

pub(crate) fn render(
    unit_type_name: &str,
    let_statements: &Vec<String>,
    struct_init_fields: &Vec<(&str, bool)>,
    tt_engine: &TinyTemplate,
) -> anyhow::Result<String> {
    tt_engine
        .render(
            "deserialize_message",
            &Context::from(
                unit_type_name,
                strip_type_name(unit_type_name)?,
                let_statements,
                struct_init_fields
                    .iter()
                    .map(|(name, is_optional)| StructInitField { name, is_optional })
                    .collect(),
            ),
        )
        .map_err(|e| {
            anyhow::Error::msg(format!(
                "TinyTemplate failed to generate deserialize_message: {e:?}"
            ))
        })
}

pub(crate) fn strip_type_name(type_name: &str) -> anyhow::Result<&str> {
    let items: Vec<&str> = type_name.split(".").collect();
    items
        .last()
        .copied()
        .ok_or_else(|| anyhow::Error::msg(format!("Cannot split type name {type_name}")))
}
