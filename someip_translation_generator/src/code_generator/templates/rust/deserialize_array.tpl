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
    let {field_name}_byte_len =
{{- if length_is_fixed }}
{length};
{{- endif }}
{{- if length_is_one_byte }}
    deserializer.deserialize_u8()?.into();
{{- endif }}
{{- if length_is_two_bytes }}
    deserializer.deserialize_u16()?.into();
{{- endif }}
{{- if length_is_four_bytes }}
    deserializer.deserialize_u32()?.try_into().map_err(|_| Err(SdvStatus::new(SdvStatusCode::OutOfRange)))?;
{{- endif }}
    let {field_name}_initial_bytes = deserializer.num_read_bytes();
    let mut {field_name} = Vec::new();
    while (deserializer.num_read_bytes() - {field_name}_initial_bytes) < {field_name}_byte_len \{
{{- if prepare_deserialize_element_instruction_needed }}
        {prepare_deserialize_element_instruction}
{{- endif }}
        {field_name}.push({deserialize_element_instruction});
    };
    if (deserializer.num_read_bytes() - {field_name}_initial_bytes) > {field_name}_byte_len \{
        return Err(SdvStatus::new(SdvStatusCode::Internal));
    };