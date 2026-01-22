<!-- Copyright (C) 2026 The Android Open Source Project

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License. -->

# Code Generator for SOME/IP to Proto Translation

The purpose of this tool is to generate the tranlation code for known SOME/IP messages.

*   Schemas of Protobuf messages used in the catalog
*   SOME/IP mapping files
*   Diagnostic declarations

## Flags

*   `catalog_path`: Path to the VSIDL catalog
*   `output_path`: Path to the output file containing the generated rust code. An existing file will be overwritten.
*   `code_gen_mode`: Specifies if the generated code is intended for a static or a dynamic library.
*   `dependency_catalog_path`: Optional. Specifies the directories of the dependent protos outside of the `catalog_path`. Each directory must contain an `Android.bp` file with a `rust_protobuf` build target to generate Rust API for the dependent protos. In the case of multi-layered dependency graph, please pass in one list with all the directories in the graph.
*   `rust_formatter`: Optional. Specifies the formatter selection for the generated code.

## Usage

The code generator creates a single rust file, containing code that can translate the known message types.

This file can be used to create a library, such as the dynamic library in the sample under `core_services/samples/some_ip/known_types/translator_lib`, where the `conversions.rs` has been generated using this tool and is used to compile the shared library `liboem_translate_known_types` used in the `core_services/samples/some_ip/config/broker_apex/Android.bp` sample. Note that in such a case, where a dynamic library is included in an apex, a corresponding `linker_config` needs to be specified that allows dynamically linking against the library (see the sample `core_services/samples/some_ip/config/broker_apex/Android.bp`).

### Modes

#### Dynamic Library

In this mode (selected by using `dyn-lib` as the `code-gen-mode` cli option) translation code is written only for the catalog messages marked as `message_translation_mode: DYNAMIC_LIBRARY`.

#### Static Library

This is intended to generate code for messages marked as `message_translation_mode: INTERPRET_AS_BYTES`.

For now, in this mode (selected by using `static-lib` as the `code-gen-mode` cli option), translation code is written for all messages. A warning is printed is printed if a message is not marked `message_translation_mode: INTERPRET_AS_BYTES`.

### Build

```bash
m someip_translation_generator
```

### Run code generator for test sample catalog

Note: most of the flags correspond to `vsidlc` flags:

```bash
someip_translation_generator -c $(path_to_catalog) -o $(path_to_output_rs_file)
```
