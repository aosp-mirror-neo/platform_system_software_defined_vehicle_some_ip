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

//! Generator for SOME/IP to Proto Translations for Known Types
//! Reads and parses a VSIDL catalog and its dependencies. Outputs code that compiles to a translator dynamic library.

use clap::ValueEnum;
use protobuf::descriptor::DescriptorProto;
use someip_config::serde_config::ffi::Endianness;
use someip_config::serde_config::ffi::LenFieldSize;
use someip_config::serde_config::ffi::SomeIpConfig;
use someip_config::serde_config::ffi::StringType;
use someip_serde_reflect::SomeIpStructTypeDefinition;
use std::collections::BTreeMap;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::simple_event::MessageTranslationMode;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::Event_type;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::event::SimpleEvent;
use vsidl_v1::someip::some_ip_mapping::some_ip_instance_implementation::Interface_type as SomeIpInterfaceType;
use vsidl_v1::someip::some_ip_mapping::Mapping_type;
use vsidl_v1_parser::Catalog;

use crate::code_generator::code_generator_impl::get_enum_to_serialize_code_fill_template;
use crate::code_generator::code_generator_impl::get_enum_to_serialize_code_template;
use crate::code_generator::code_generator_impl::strip_type_name;
use crate::code_generator::code_generator_impl::CodeGenerator;
use crate::type_parser::TypeParser;

const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
    endianness: Endianness::BigEndian,
    string_type: StringType::UTF_8,
    len_field_size: LenFieldSize::ZeroBytes,
};

/// Enum to control the generation mode
#[derive(ValueEnum, Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) enum CodeGenMode {
    /// Generate code for dynamic library.
    /// This creates code for simple event messages tagged with `message_translation_mode: DYNAMIC_LIBRARY`
    DynLib,
    /// This creates code for simple event messages tagged with `message_translation_mode: INTERPRET_AS_BYTES`
    StaticLib,
}

pub(crate) fn generate_library_code(
    catalog: &Catalog,
    mode: &CodeGenMode,
) -> anyhow::Result<String> {
    let package_unit_type_to_some_ip_def = get_package_unit_type_to_some_ip_def(catalog, mode);
    let package_unit_type_to_proto_def = get_package_unit_type_to_proto_def(catalog);
    let unit_type_to_proto_file = get_unit_type_to_proto_file(catalog);
    let unit_type_to_full_name = get_unit_type_to_full_name(catalog);
    let enum_to_proto_file = get_enum_to_proto_file(catalog);
    let enum_to_serialize_code = get_enum_to_serialize_code_template(catalog)?;

    let mut unit_types_processed = Vec::new();

    let mut code_generator_errors = Vec::new();
    let mut header_use_lines = "".to_owned();
    let mut code_output_messages = "".to_owned();
    let mut code_output_enums = "".to_owned();

    for (package, unit_type_to_some_ip_def) in package_unit_type_to_some_ip_def.iter() {
        let Some(unit_type_to_proto_def) = package_unit_type_to_proto_def.get(package) else {
            continue;
            //return Err(anyhow::Error::msg(format!("missing package {}", package)));
        };

        let mut code_generator = CodeGenerator::new(unit_type_to_proto_def);

        for (unit_type_name, some_ip_def) in unit_type_to_some_ip_def.iter() {
            let generated_code =
                code_generator.generate_code_for_message_def(unit_type_name.as_str(), some_ip_def);
            if let Ok(generated_code) = generated_code {
                code_output_messages.push_str(generated_code.as_str());
                unit_types_processed.push(unit_type_name.as_str());
            } else {
                code_generator_errors.push(generated_code);
            }
        }

        let mut header_use_statements = BTreeMap::new();
        for type_name in code_generator.get_processed_unit_types().iter() {
            let Some(proto_file) = unit_type_to_proto_file.get(*type_name) else {
                continue;
            };
            let Ok(type_name) = strip_type_name(type_name) else {
                continue;
            };

            header_use_statements.entry(proto_file).or_insert(Vec::new()).push(type_name);
        }
        for (enum_full_name, someip_size_type) in code_generator.get_processed_enums().iter() {
            let Some(proto_file) = enum_to_proto_file.get(*enum_full_name) else {
                continue;
            };
            let Ok(enum_name) = strip_type_name(enum_full_name) else {
                continue;
            };

            header_use_statements.entry(proto_file).or_insert(Vec::new()).push(enum_name);

            let Some(enum_code) = enum_to_serialize_code.get(*enum_full_name) else {
                continue;
            };

            let enum_code =
                get_enum_to_serialize_code_fill_template(enum_code.as_str(), someip_size_type);
            let Ok(enum_code) = enum_code else {
                code_generator_errors.push(enum_code);
                continue;
            };

            code_output_enums.push_str(enum_code.as_str());
        }
        for ((proto_file, proto_crate), type_names) in header_use_statements.iter() {
            // remove the .proto extension and the folder prefix
            let proto_file = remove_folder_prefix(proto_file).unwrap_or(proto_file);
            let proto_file = proto_file[..proto_file.len() - 6].to_owned();

            let mut use_line = "use ".to_owned() + proto_crate + "::" + proto_file.as_str() + "::{";
            for type_name in type_names.iter() {
                use_line.push_str(type_name);
                use_line.push_str(", ");
            }
            use_line.pop();
            use_line.pop();
            use_line.push_str("};\n");
            header_use_lines.push_str(use_line.as_str());
        }
    }

    for e in code_generator_errors {
        eprintln!("WARNING: Ignored Error: {e:?}");
    }

    // merge generated code for messages and enums
    code_output_messages.push_str(code_output_enums.as_str());

    CodeGenerator::generate_code_for_file(
        header_use_lines.as_str(),
        &unit_types_processed,
        &unit_type_to_full_name,
        code_output_messages.as_str(),
    )
}

fn get_package_unit_type_to_some_ip_def(
    catalog: &Catalog,
    mode: &CodeGenMode,
) -> BTreeMap<String, BTreeMap<String, SomeIpStructTypeDefinition>> {
    let mut package_unit_type_to_some_ip_def = BTreeMap::new();
    for (_path, entry) in catalog.catalog_vsidl_files.iter() {
        for some_ip_mapping in entry.some_ip_mapping.iter() {
            let Some(Mapping_type::SomeipServiceMapping(ref someip_service_mapping)) =
                some_ip_mapping.mapping_type
            else {
                continue;
            };
            for instance_impl in someip_service_mapping.instance_impl.iter() {
                let Some(SomeIpInterfaceType::OfferedEvent(ref event)) =
                    instance_impl.interface_type
                else {
                    continue;
                };
                let Some(Event_type::SimpleEvent(ref event)) = event.event_type else {
                    continue;
                };

                if !message_filter(event, mode) {
                    continue;
                };

                let Some(sdv_unit_type) = event.sdv_unit_type.as_ref() else {
                    continue;
                };
                let Ok(Some(someip_struct_type_definition)) =
                    TypeParser::parse(&DEFAULT_SOMEIP_CONFIG, event.type_definition.as_ref())
                else {
                    continue;
                };

                let key =
                    ".".to_owned() + &sdv_unit_type.package_name + "." + &sdv_unit_type.type_name;
                package_unit_type_to_some_ip_def
                    .entry(sdv_unit_type.package_name.clone())
                    .or_insert(BTreeMap::new())
                    .insert(key, someip_struct_type_definition);
            }
        }
    }
    package_unit_type_to_some_ip_def
}

fn get_unit_type_to_full_name(catalog: &Catalog) -> BTreeMap<String, String> {
    let mut unit_type_to_full_name = BTreeMap::new();
    for (_path, entry) in catalog.catalog_vsidl_files.iter() {
        for some_ip_mapping in entry.some_ip_mapping.iter() {
            let Some(Mapping_type::SomeipServiceMapping(ref someip_service_mapping)) =
                some_ip_mapping.mapping_type
            else {
                continue;
            };
            for instance_impl in someip_service_mapping.instance_impl.iter() {
                let Some(SomeIpInterfaceType::OfferedEvent(ref event)) =
                    instance_impl.interface_type
                else {
                    continue;
                };
                let Some(Event_type::SimpleEvent(ref event)) = event.event_type else {
                    continue;
                };

                let Some(sdv_unit_type) = event.sdv_unit_type.as_ref() else {
                    continue;
                };

                let key =
                    ".".to_owned() + &sdv_unit_type.package_name + "." + &sdv_unit_type.type_name;
                unit_type_to_full_name.insert(
                    key,
                    sdv_unit_type.package_name.clone()
                        + "."
                        + sdv_unit_type.service_bundle_name.as_str()
                        + "#"
                        + sdv_unit_type.type_name.as_str(),
                );
            }
        }
    }
    unit_type_to_full_name
}

fn get_package_unit_type_to_proto_def(
    catalog: &Catalog,
) -> BTreeMap<String, BTreeMap<String, &DescriptorProto>> {
    let mut package_unit_type_to_proto_def = BTreeMap::new();
    for (_, file_descriptor_proto) in catalog.proto_descriptors.iter() {
        let Some(ref package) = file_descriptor_proto.package else {
            continue;
        };
        let unit_type_to_proto_def =
            package_unit_type_to_proto_def.entry(package.clone()).or_insert(BTreeMap::new());

        for msg in file_descriptor_proto.message_type.iter() {
            let Some(ref type_name) = msg.name else {
                continue;
            };
            let key = ".".to_owned() + package.as_str() + "." + type_name;
            unit_type_to_proto_def.insert(key, msg);
        }
    }
    package_unit_type_to_proto_def
}

fn get_unit_type_to_proto_file(catalog: &Catalog) -> BTreeMap<String, (String, String)> {
    let mut unit_type_to_proto_file = BTreeMap::new();
    for (proto_metadata, file_descriptor_proto) in catalog.proto_descriptors.iter() {
        let Some(proto_crate_name) = proto_metadata.crate_name.as_ref() else {
            continue;
        };
        let Some(ref package) = file_descriptor_proto.package else {
            continue;
        };
        let Some(ref proto_file_name) = file_descriptor_proto.name else {
            continue;
        };

        for msg in file_descriptor_proto.message_type.iter() {
            let Some(ref type_name) = msg.name else {
                continue;
            };
            let key = ".".to_owned() + package.as_str() + "." + type_name;
            unit_type_to_proto_file
                .insert(key.clone(), (proto_file_name.clone(), proto_crate_name.clone()));
        }
    }
    unit_type_to_proto_file
}

fn get_enum_to_proto_file(catalog: &Catalog) -> BTreeMap<String, (String, String)> {
    let mut enum_to_proto_file = BTreeMap::new();
    for (proto_metadata, file_descriptor_proto) in catalog.proto_descriptors.iter() {
        let Some(proto_crate_name) = proto_metadata.crate_name.as_ref() else {
            continue;
        };
        let Some(ref package) = file_descriptor_proto.package else {
            continue;
        };
        let Some(ref proto_file_name) = file_descriptor_proto.name else {
            continue;
        };

        for enum_type in file_descriptor_proto.enum_type.iter() {
            let Some(ref enum_name) = enum_type.name else {
                continue;
            };
            let key = ".".to_owned() + package.as_str() + "." + enum_name;
            enum_to_proto_file
                .insert(key.clone(), (proto_file_name.clone(), proto_crate_name.clone()));
        }
    }
    enum_to_proto_file
}

fn remove_folder_prefix(input: &str) -> anyhow::Result<&str> {
    let items: Vec<&str> = input.split("/").collect();
    items
        .last()
        .copied()
        .ok_or_else(|| anyhow::Error::msg(format!("Cannot remove folder prefix {input}")))
}

fn message_filter(event: &SimpleEvent, mode: &CodeGenMode) -> bool {
    match mode {
        CodeGenMode::DynLib => {
            let Some(translation_mode) = event.message_translation_mode.as_ref() else {
                return false;
            };
            let Ok(translation_mode) = translation_mode.enum_value() else {
                return false;
            };
            if translation_mode != MessageTranslationMode::DYNAMIC_LIBRARY {
                return false;
            };
        }
        CodeGenMode::StaticLib => {
            let Some(translation_mode) = event.message_translation_mode.as_ref() else {
                eprintln!("WARNING: translation_mode not found. Messages should be marked as MessageTranslationMode::INTERPRET_AS_BYTES");
                return true;
            };
            let Ok(translation_mode) = translation_mode.enum_value() else {
                eprintln!("WARNING: translation_mode not found. Messages should be marked as MessageTranslationMode::INTERPRET_AS_BYTES");
                return true;
            };
            if translation_mode != MessageTranslationMode::INTERPRET_AS_BYTES {
                eprintln!("WARNING: message is not marked as INTERPRET_AS_BYTES");
                return true;
            };
        }
    };
    true
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn build_catalog() -> Catalog {
        // no need to provide a path to vsidl syntax, as it is already included in the test's `data`
        vsidl_v1_parser::read_catalog(Path::new("tests/catalog/"), &[], Path::new("./"))
            .expect("Unable to read catalog")
    }

    fn test_fixture_generate_someip_translation_code_against_reference_file(
        rust_formatter: formatter::Format,
        mode: CodeGenMode,
        reference_file: &str,
    ) {
        let catalog = build_catalog();
        let mut rust_formatter = formatter::get_rust_formatter(rust_formatter);
        let generated_code = rust_formatter
            .format(generate_library_code(&catalog, &mode).unwrap().as_str())
            .unwrap();

        let existing_content = fs::read(reference_file).unwrap();
        assert_eq!(String::from_utf8(existing_content).unwrap(), generated_code);
    }

    // NOTE: The following tests panics in CI
    /*
    #[test]
    fn test_generate_someip_translation_code_against_reference_file_dyn_rust_fmt() {
        test_fixture_generate_someip_translation_code_against_reference_file(
            formatter::Format::RustFmt,
            CodeGenMode::DynLib,
            "tests/reference/generated_rust_fmt_dyn.ref",
        );
    }
    */

    #[test]
    fn test_generate_someip_translation_code_against_reference_file_dyn_pretty_please() {
        test_fixture_generate_someip_translation_code_against_reference_file(
            formatter::Format::PrettyPlease,
            CodeGenMode::DynLib,
            "tests/reference/generated_pretty_please_dyn.ref",
        );
    }

    #[test]
    fn test_generate_someip_translation_code_against_reference_file_dyn_none() {
        test_fixture_generate_someip_translation_code_against_reference_file(
            formatter::Format::None,
            CodeGenMode::DynLib,
            "tests/reference/generated_none_dyn.ref",
        );
    }

    // NOTE: The following tests panics in CI
    /*
    #[test]
    fn test_generate_someip_translation_code_against_reference_file_static_rust_fmt() {
        test_fixture_generate_someip_translation_code_against_reference_file(
            formatter::Format::RustFmt,
            CodeGenMode::StaticLib,
            "tests/reference/generated_rust_fmt_static.ref",
        );
    }
    */

    #[test]
    fn test_generate_someip_translation_code_against_reference_file_static_pretty_please() {
        test_fixture_generate_someip_translation_code_against_reference_file(
            formatter::Format::PrettyPlease,
            CodeGenMode::StaticLib,
            "tests/reference/generated_pretty_please_static.ref",
        );
    }

    #[test]
    fn test_generate_someip_translation_code_against_reference_file_static_none() {
        test_fixture_generate_someip_translation_code_against_reference_file(
            formatter::Format::None,
            CodeGenMode::StaticLib,
            "tests/reference/generated_none_static.ref",
        );
    }
}
