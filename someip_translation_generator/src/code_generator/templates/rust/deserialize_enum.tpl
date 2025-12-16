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
pub fn deserialize_{enum_name_lowercase}(
    deserializer: &mut dyn ISomeIpDeserializer
) -> SdvResult<{enum_name}> \{
    let enum_value = deserializer.deserialize_{insert_type_placeholder};
    match enum_value \{
{{- for value in values }}
        {value.number} => Ok({enum_name}::{value.name}),
{{- endfor }}
        _ => Err(SdvStatus::new(SdvStatusCode::NotFound)),
    }
}