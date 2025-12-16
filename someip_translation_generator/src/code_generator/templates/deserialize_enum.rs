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

use protobuf::descriptor::EnumValueDescriptorProto;
use serde_derive::Serialize;
use tinytemplate::TinyTemplate;

#[derive(Serialize)]
struct EnumValue<'a> {
    number: i32,
    name: &'a str,
}

#[derive(Serialize)]
struct Context<'a> {
    enum_name: &'a str,
    enum_name_lowercase: String,
    values: Vec<EnumValue<'a>>,
    insert_type_placeholder: &'a str,
}

impl<'a> Context<'a> {
    pub(self) fn from(
        enum_name: &'a str,
        values: Vec<EnumValue<'a>>,
        insert_type_placeholder: &'a str,
    ) -> Self {
        Context {
            enum_name,
            enum_name_lowercase: enum_name.to_lowercase(),
            values,
            insert_type_placeholder,
        }
    }
}

pub(crate) fn render(
    enum_name: &str,
    values: &Vec<EnumValueDescriptorProto>,
    insert_type_placeholder: &str,
    tt_engine: &TinyTemplate,
) -> anyhow::Result<String> {
    let mut enum_values = Vec::new();
    for val in values.iter() {
        let Some(number) = val.number else {
            return Err(anyhow::Error::msg(format!("Missing number in {values:?}")));
        };
        let Some(ref name) = val.name else {
            return Err(anyhow::Error::msg(format!("Missing name in {values:?}")));
        };

        enum_values.push(EnumValue { number, name });
    }

    Ok(tt_engine
        .render("deserialize_enum", &Context::from(enum_name, enum_values, insert_type_placeholder))
        .unwrap())
}
