{#-
 Copyright (C) 2026 The Android Open Source Project

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License.
-#}

#[inline(always)]
pub fn deserialize{unit_type_name_lowercase}(
    deserializer: &mut dyn ISomeIpDeserializer
) -> SdvResult<{unit_type_name_stripped}> \{
{{- for let_statement in let_statements }}
{let_statement}
{{- endfor }}
    Ok({unit_type_name_stripped} \{
{{- for struct_init_field in struct_init_fields }}
{{- if struct_init_field.is_optional }}
        {struct_init_field.name}: Some({struct_init_field.name}),
{{- else }}
        {struct_init_field.name},
{{- endif }}
{{- endfor }}
    ..Default::default()
    })
}