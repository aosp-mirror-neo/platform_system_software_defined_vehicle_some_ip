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

//! Generator implementation for SOME/IP to Proto Translations for Known Types

use protobuf::descriptor::field_descriptor_proto;
use protobuf::descriptor::field_descriptor_proto::Type::*;
use protobuf::descriptor::DescriptorProto;
use protobuf::descriptor::FieldDescriptorProto;
use protobuf::EnumOrUnknown;
use someip_config::serde_config::ffi::LenFieldSize;
use someip_serde_reflect::SomeIpArrayTypeDefinition;
use someip_serde_reflect::SomeIpMessageTypeDefinition;
use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;
use someip_serde_reflect::SomeIpPrimitiveTypeDefinition::*;
use someip_serde_reflect::SomeIpStringTypeDefinition;
use someip_serde_reflect::SomeIpStructFieldTypeDefinition;
use someip_serde_reflect::SomeIpStructTypeDefinition;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use tinytemplate::format_unescaped;
use tinytemplate::TinyTemplate;
use vsidl_v1_parser::Catalog;

use crate::code_generator::code_generator_impl_rust::*;
use crate::code_generator::templates::deserialize_array;
use crate::code_generator::templates::deserialize_enum;
use crate::code_generator::templates::deserialize_message;
use crate::code_generator::templates::deserialize_string;
use crate::code_generator::templates::global;

// The output strings are incomplete, the "<INSERT_TYPE>" substring needs to be replaced
pub(crate) fn get_enum_to_serialize_code_template(
    catalog: &Catalog,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut enum_to_serialize_code = BTreeMap::new();
    for (_, file_descriptor_proto) in catalog.proto_descriptors.iter() {
        let Some(ref package) = file_descriptor_proto.package else {
            continue;
        };

        for enum_type in file_descriptor_proto.enum_type.iter() {
            let Some(ref enum_name) = enum_type.name else {
                continue;
            };
            let key = ".".to_owned() + package.as_str() + "." + enum_name;

            let mut tt: TinyTemplate<'_> = TinyTemplate::new();
            tt.add_template("deserialize_enum", DESERIALIZE_ENUM_TPL)?;
            tt.set_default_formatter(&format_unescaped);
            let Ok(serialize_enum_function_code) =
                deserialize_enum::render(enum_name, &enum_type.value, INSERT_TYPE_PLACEHOLDER, &tt)
            else {
                continue;
            };

            enum_to_serialize_code.insert(key, serialize_enum_function_code);
        }
    }
    Ok(enum_to_serialize_code)
}

pub(crate) fn get_enum_to_serialize_code_fill_template(
    enum_code: &str,
    someip_size_type: &SomeIpPrimitiveTypeDefinition,
) -> anyhow::Result<String> {
    match someip_size_type {
        SomeIpPrimitiveTypeDefinition::I8 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_I8))
        }
        SomeIpPrimitiveTypeDefinition::I16 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_I16))
        }
        SomeIpPrimitiveTypeDefinition::I32 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_I32))
        }
        SomeIpPrimitiveTypeDefinition::I64 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_I64))
        }
        SomeIpPrimitiveTypeDefinition::U8 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_U8))
        }
        SomeIpPrimitiveTypeDefinition::U16 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_U16))
        }
        SomeIpPrimitiveTypeDefinition::U32 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_U32))
        }
        SomeIpPrimitiveTypeDefinition::U64 => {
            Ok(enum_code.replace(INSERT_TYPE_PLACEHOLDER, PLACEHOLDER_FOR_U64))
        }
        _ => Err(anyhow::Error::msg(format!("Unsupported someip_size_type {someip_size_type:?}",))),
    }
}

pub(crate) struct CodeGenerator<'a> {
    unit_type_to_proto_def: &'a BTreeMap<String, &'a DescriptorProto>,
    pending_someip_structs: VecDeque<(&'a str, &'a SomeIpStructTypeDefinition)>,
    processed_someip_structs: BTreeSet<&'a str>,
    processed_enums: BTreeMap<&'a str, &'a SomeIpPrimitiveTypeDefinition>,
}

impl<'a> CodeGenerator<'a> {
    pub(crate) fn new(
        unit_type_to_proto_def: &'a BTreeMap<String, &'a DescriptorProto>,
    ) -> CodeGenerator<'a> {
        CodeGenerator {
            unit_type_to_proto_def,
            pending_someip_structs: VecDeque::new(),
            processed_someip_structs: BTreeSet::new(),
            processed_enums: BTreeMap::new(),
        }
    }

    pub(crate) fn get_processed_unit_types(&self) -> &BTreeSet<&'a str> {
        &self.processed_someip_structs
    }

    pub(crate) fn get_processed_enums(
        &self,
    ) -> &BTreeMap<&'a str, &'a SomeIpPrimitiveTypeDefinition> {
        &self.processed_enums
    }

    pub(crate) fn generate_code_for_message_def(
        &mut self,
        unit_type_name: &'a str,
        some_ip_def: &'a SomeIpStructTypeDefinition,
    ) -> anyhow::Result<String> {
        self.pending_someip_structs.push_back((unit_type_name, some_ip_def));

        let mut out = "".to_owned();

        while let Some((unit_type_name, some_ip_def)) = self.pending_someip_structs.pop_front() {
            if self.processed_someip_structs.contains(&unit_type_name) {
                continue;
            }

            let Some(proto_def) = self.unit_type_to_proto_def.get(unit_type_name) else {
                return Err(anyhow::Error::msg(format!(
                    "missing unit {} in {:?}",
                    unit_type_name, self.unit_type_to_proto_def
                )));
            };

            let (let_statements, struct_init_fields) =
                self.generate_code_for_protobuf_message(some_ip_def, proto_def)?;

            let mut tt: TinyTemplate<'_> = TinyTemplate::new();
            tt.add_template("deserialize_message", DESERIALIZE_MESSAGE_TPL)?;
            tt.set_default_formatter(&format_unescaped);
            out.push_str(
                deserialize_message::render(
                    unit_type_name,
                    &let_statements,
                    &struct_init_fields,
                    &tt,
                )?
                .as_str(),
            );

            self.processed_someip_structs.insert(unit_type_name);
        }
        Ok(out)
    }

    // first return is the code with the let statements
    // second return is a list of the fields (for struct creation)
    fn generate_code_for_protobuf_message(
        &mut self,
        some_ip_def: &'a SomeIpStructTypeDefinition,
        proto_def: &'a DescriptorProto,
    ) -> anyhow::Result<(Vec<String>, Vec<(&str, bool)>)> {
        let mut let_statements = Vec::new();
        let mut struct_init_fields = Vec::new();
        for someip_field in some_ip_def.fields.iter() {
            let Some(proto_field) =
                &proto_def.field.iter().find(|&proto| match proto.name.as_ref() {
                    Some(cmp) => someip_field.field_name.as_str() == cmp,
                    None => false,
                })
            else {
                return Err(anyhow::Error::msg(format!(
                    "did not find proto field counterpart for {}",
                    someip_field.field_name.as_str()
                )));
            };

            let_statements.push(self.generate_code_for_message_field(someip_field, proto_field)?);

            let proto_is_optional = proto_field.proto3_optional.unwrap_or(false);
            struct_init_fields.push((someip_field.field_name.as_str(), proto_is_optional));
        }
        Ok((let_statements, struct_init_fields))
    }

    fn generate_code_for_message_field(
        &mut self,
        someip_field: &'a SomeIpStructFieldTypeDefinition,
        proto_field: &'a FieldDescriptorProto,
    ) -> anyhow::Result<String> {
        let Some(ref proto_field_name) = proto_field.name else {
            return Err(anyhow::Error::msg("proto field name not available"));
        };
        if *proto_field_name != someip_field.field_name {
            return Err(anyhow::Error::msg("name mismatch"));
        }
        let Some(ref proto_field_type) = proto_field.type_ else {
            return Err(anyhow::Error::msg("field type not available"));
        };
        let field_name = someip_field.field_name.as_str();

        match someip_field.field_type {
            SomeIpMessageTypeDefinition::PRIMITIVE(ref primitive) => self
                .generate_code_for_primitive(
                    field_name,
                    primitive,
                    proto_field_type,
                    &proto_field.type_name,
                ),
            SomeIpMessageTypeDefinition::ARRAY(ref array) => {
                self.generate_code_for_array(field_name, array, proto_field)
            }
            SomeIpMessageTypeDefinition::STRUCT(ref someip_struct) => {
                let Some(ref type_name) = proto_field.type_name else {
                    return Err(anyhow::Error::msg("field type_name not available"));
                };
                self.pending_someip_structs.push_back((type_name, someip_struct));
                Ok(deserialize_substruct_let_instruction(field_name, type_name))
            }
            SomeIpMessageTypeDefinition::STRING(ref someip_string) => {
                self.generate_code_for_string(field_name, someip_string)
            }
        }
    }

    fn generate_code_for_array(
        &mut self,
        field_name: &str,
        array: &'a SomeIpArrayTypeDefinition,
        proto_field: &'a FieldDescriptorProto,
    ) -> anyhow::Result<String> {
        let length_type = match array.fixed_length {
            Some(len) => deserialize_array::LengthType::FixedLength(len),
            None => match array.length_field_size {
                LenFieldSize::OneByte => deserialize_array::LengthType::OneByte,
                LenFieldSize::TwoBytes => deserialize_array::LengthType::TwoBytes,
                LenFieldSize::FourBytes => deserialize_array::LengthType::FourBytes,
                _ => {
                    return Err(anyhow::Error::msg("cannot resolve LengthFieldSize"));
                }
            },
        };
        static STRING_ELEMENT_SUFFIX: &str = "_element";
        static ARRAY_ELEMENT_SUFFIX: &str = "_inner";

        let prepare_deserialize_element_instruction = match *array.array_type {
            SomeIpMessageTypeDefinition::STRING(ref someip_string) => {
                let field_element_name = field_name.to_owned() + STRING_ELEMENT_SUFFIX;
                self.generate_code_for_string(field_element_name.as_str(), someip_string)?
            }
            SomeIpMessageTypeDefinition::ARRAY(ref someip_array) => {
                let inner_name = field_name.to_owned() + ARRAY_ELEMENT_SUFFIX;
                self.generate_code_for_array(inner_name.as_str(), someip_array, proto_field)?
            }
            _ => "".to_owned(),
        };

        let deserialize_element_instruction = match *array.array_type {
            SomeIpMessageTypeDefinition::PRIMITIVE(ref primitive) => {
                let Some(ref proto_field_type) = proto_field.type_ else {
                    return Err(anyhow::Error::msg("field type not available"));
                };
                self.generate_deserialize_instruction(
                    primitive,
                    proto_field_type,
                    &proto_field.type_name,
                )?
            }
            SomeIpMessageTypeDefinition::STRUCT(_) => {
                let Some(ref type_name) = proto_field.type_name else {
                    return Err(anyhow::Error::msg("field type_name not available"));
                };
                deserialize_substruct_instruction(type_name)
            }
            SomeIpMessageTypeDefinition::STRING(_) => field_name.to_owned() + STRING_ELEMENT_SUFFIX,
            SomeIpMessageTypeDefinition::ARRAY(_) => field_name.to_owned() + ARRAY_ELEMENT_SUFFIX,
        };

        if let SomeIpMessageTypeDefinition::STRUCT(ref someip_struct) = *array.array_type {
            let Some(ref type_name) = proto_field.type_name else {
                return Err(anyhow::Error::msg("field type_name not available"));
            };
            self.pending_someip_structs.push_back((type_name, someip_struct));
        };

        let mut tt: TinyTemplate<'_> = TinyTemplate::new();
        tt.add_template("deserialize_array", DESERIALIZE_ARRAY_TPL)?;
        tt.set_default_formatter(&format_unescaped);
        deserialize_array::render(
            field_name,
            length_type,
            prepare_deserialize_element_instruction.as_str(),
            deserialize_element_instruction.as_str(),
            &tt,
        )
    }

    fn generate_code_for_primitive(
        &mut self,
        field_name: &str,
        someip_type: &'a SomeIpPrimitiveTypeDefinition,
        proto_type: &EnumOrUnknown<field_descriptor_proto::Type>,
        proto_type_name: &'a Option<String>,
    ) -> anyhow::Result<String> {
        Ok(deserialize_primitive_let_instruction(
            field_name,
            self.generate_deserialize_instruction(someip_type, proto_type, proto_type_name)?
                .as_str(),
        ))
    }

    fn generate_code_for_string(
        &mut self,
        field_name: &str,
        someip_string: &SomeIpStringTypeDefinition,
    ) -> anyhow::Result<String> {
        let length_type = match someip_string.fixed_length {
            Some(len) => deserialize_string::LengthType::FixedLength(len),
            None => match someip_string.length_field_size {
                LenFieldSize::OneByte => deserialize_string::LengthType::OneByte,
                LenFieldSize::TwoBytes => deserialize_string::LengthType::TwoBytes,
                LenFieldSize::FourBytes => deserialize_string::LengthType::FourBytes,
                _ => {
                    return Err(anyhow::Error::msg("cannot resolve LengthFieldSize"));
                }
            },
        };

        let mut tt: TinyTemplate<'_> = TinyTemplate::new();
        tt.add_template("deserialize_string", DESERIALIZE_STRING_TPL)?;
        tt.set_default_formatter(&format_unescaped);
        deserialize_string::render(field_name, length_type, &tt)
    }

    fn generate_enum_deserialize_instruction(
        &mut self,
        proto_type_name: &'a Option<String>,
        someip_type: &'a SomeIpPrimitiveTypeDefinition,
    ) -> anyhow::Result<String> {
        let proto_type_name = proto_type_name
            .as_ref()
            .ok_or(anyhow::Error::msg("TYPE_ENUM does not contain proto_type_name"))?;
        self.processed_enums.insert(proto_type_name.as_str(), someip_type);
        Ok(deserialize_enum_instruction(strip_type_name(proto_type_name.as_str())?))
    }

    fn generate_deserialize_instruction(
        &mut self,
        someip_type: &'a SomeIpPrimitiveTypeDefinition,
        proto_type: &EnumOrUnknown<field_descriptor_proto::Type>,
        proto_type_name: &'a Option<String>,
    ) -> anyhow::Result<String> {
        let Ok(proto_type) = proto_type.enum_value() else {
            return Err(anyhow::Error::msg("proto type enum invalid"));
        };

        match (someip_type, proto_type) {
            (BOOLEAN, TYPE_BOOL) => Ok(INSTRUCTION_BOOLEAN_TYPE_BOOL.to_owned()),
            (I8, TYPE_INT64 | TYPE_UINT64 | TYPE_INT32 | TYPE_UINT32) => {
                Ok(INSTRUCTION_I8.to_owned())
            }
            (I16, TYPE_INT64 | TYPE_UINT64 | TYPE_INT32 | TYPE_UINT32) => {
                Ok(INSTRUCTION_I16.to_owned())
            }
            (I32, TYPE_INT32) => Ok(INSTRUCTION_I32_TYPE_INT32.to_owned()),
            (I32, TYPE_INT64 | TYPE_UINT64 | TYPE_UINT32) => Ok(INSTRUCTION_I32.to_owned()),
            (I64, TYPE_INT64) => Ok(INSTRUCTION_I64_TYPE_INT64.to_owned()),
            (I64, TYPE_UINT64 | TYPE_INT32 | TYPE_UINT32) => Ok(INSTRUCTION_I64.to_owned()),
            (U8, TYPE_BYTES) => Ok(INSTRUCTION_U8_TYPE_BYTES.to_owned()),
            (U8, TYPE_INT64 | TYPE_UINT64 | TYPE_INT32 | TYPE_UINT32) => {
                Ok(INSTRUCTION_U8.to_owned())
            }
            (U16, TYPE_INT64 | TYPE_UINT64 | TYPE_INT32 | TYPE_UINT32) => {
                Ok(INSTRUCTION_U16.to_owned())
            }
            (U32, TYPE_UINT32) => Ok(INSTRUCTION_U32_TYPE_UINT32.to_owned()),
            (U32, TYPE_INT64 | TYPE_UINT64 | TYPE_INT32) => Ok(INSTRUCTION_U32.to_owned()),
            (U64, TYPE_UINT64) => Ok(INSTRUCTION_U64_TYPE_UINT64.to_owned()),
            (U64, TYPE_INT64 | TYPE_INT32 | TYPE_UINT32) => Ok(INSTRUCTION_U64.to_owned()),
            (F32, TYPE_FLOAT) => Ok(INSTRUCTION_F32_FLOAT.to_owned()),
            (F32, TYPE_DOUBLE) => Ok(INSTRUCTION_F32_DOUBLE.to_owned()),
            (F64, TYPE_FLOAT) => Ok(INSTRUCTION_F64_FLOAT.to_owned()),
            (F64, TYPE_DOUBLE) => Ok(INSTRUCTION_F64_DOUBLE.to_owned()),
            (I8 | I16 | I32 | I64 | U8 | U16 | U32 | U64, TYPE_ENUM) => {
                self.generate_enum_deserialize_instruction(proto_type_name, someip_type)
            }
            (_, _) => Err(anyhow::Error::msg(format!(
                "type conversion not supported {someip_type:?} {proto_type:?}"
            ))),
        }
    }

    fn get_name_to_function_pairs_for_top_function(
        unit_types_processed: &Vec<&str>,
        unit_type_to_full_name: &BTreeMap<String, String>,
    ) -> anyhow::Result<Vec<(String, String)>> {
        let mut code_for_top_function = Vec::new();
        for unit_type_name in unit_types_processed.iter() {
            code_for_top_function.push((
                unit_type_to_full_name
                    .get(*unit_type_name)
                    .ok_or(anyhow::Error::msg(format!(
                        "Cannot find full name (that includes service bundle) for {unit_type_name}"
                    )))?
                    .clone(),
                unit_type_name.replace(".", "_").to_lowercase(),
            ));
        }
        Ok(code_for_top_function)
    }

    pub(crate) fn generate_code_for_file(
        header_use_lines: &str,
        unit_types_processed: &Vec<&str>,
        unit_type_to_full_name: &BTreeMap<String, String>,
        code_output_messages: &str,
    ) -> anyhow::Result<String> {
        let mut tt: TinyTemplate<'_> = TinyTemplate::new();
        tt.add_template("global", GLOBAL_TPL)?;
        tt.set_default_formatter(&format_unescaped);
        global::render(
            header_use_lines,
            &Self::get_name_to_function_pairs_for_top_function(
                unit_types_processed,
                unit_type_to_full_name,
            )?,
            code_output_messages,
            &tt,
        )
    }
}

pub(crate) fn strip_type_name(type_name: &str) -> anyhow::Result<&str> {
    let items: Vec<&str> = type_name.split(".").collect();
    items
        .last()
        .copied()
        .ok_or_else(|| anyhow::Error::msg(format!("Cannot split type name {type_name}")))
}
