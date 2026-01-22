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

//! CLI Binary Executable
//! Generator for SOME/IP to Proto Translations for Known Types
//! Reads and parses a VSIDL catalog and its dependencies. Outputs code that compiles to a translator dynamic library.

mod code_generator;
mod type_parser;

use clap::Parser;
use formatter::Format;
use std::fs;
use std::path::{Path, PathBuf};

use crate::code_generator::generate_library_code::generate_library_code;
use crate::code_generator::generate_library_code::CodeGenMode;

/// Generates SOME/IP translation library code from .vsidl and .proto files.
#[derive(Parser)]
pub struct Cli {
    /// Path to catalog directory, i.e., the directory that contains .vsidl files
    #[arg(short, long)]
    catalog_path: PathBuf,

    /// Path to parent directory of `generated_rs` folder in which generated files are placed.
    #[arg(short, long)]
    output_path: PathBuf,

    /// Optional. Specifies the directories of the dependent protos outside of the `catalog_path`. Each directory must contain an `Android.bp` file with a `rust_protobuf` build target to generate Rust API for the dependent protos. In the case of multi-layered dependency graph, please pass in one list with all the directories in the graph.
    #[arg(short, long, num_args(1..), value_delimiter = ',')]
    dependency_catalog_path: Vec<PathBuf>,

    /// Optional. Defines what tool is used to format rust code. By default will use rustfmt.
    /// - rust-fmt relies on either ANDROID_BUILD_TOP to be set or both RUSTFMT_PATH (executable) and RUSTFMT_TOML_PATH (format file)
    /// - pretty-please does not requires any additional setting to run
    /// - none does not requires any additional settings and will not apply any formatting to the output
    #[arg(long, value_enum, default_value_t = Format::RustFmt)]
    rust_formatter: Format,

    /// Specifies if the generated code is intended for a static or a dynamic library.
    #[arg(long, value_enum)]
    code_gen_mode: CodeGenMode,
}

#[allow(unreachable_code)]
#[allow(unused_variables)]
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    eprintln!("Parsing catalog from folder '{}'.", &cli.catalog_path.display());

    let dependencies: Vec<&std::path::Path> =
        cli.dependency_catalog_path.iter().map(PathBuf::as_path).collect();
    let catalog = vsidl_v1_parser::read_catalog(
        cli.catalog_path.as_path(),
        &dependencies,
        vsidl_v1_parser::get_vsidl_syntax_default_path()?.as_path(),
    )?;

    let mut rust_formatter = formatter::get_rust_formatter(cli.rust_formatter);
    let generated_code =
        rust_formatter.format(generate_library_code(&catalog, &cli.code_gen_mode)?.as_str())?;
    write_bytes_to_file(generated_code.as_str(), cli.output_path.as_path())?;

    Ok(())
}

/// Writes the content to the file with path, if content is different
fn write_bytes_to_file(content: &str, path: &Path) -> anyhow::Result<()> {
    // Skips if the same content already exists in the path.
    if let Ok(existing_content) = fs::read(path) {
        if existing_content.as_slice() == (std::convert::AsRef::<[u8]>::as_ref(content)) {
            return Ok(());
        }
    }

    Ok(fs::write(path, content)?)
}
