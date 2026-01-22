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

use lazy_static::lazy_static;
use protobuf::descriptor::field_descriptor_proto::Type;
use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;
use std::collections::{HashMap, HashSet};

lazy_static! {
    pub(crate) static ref VALID_PRIMITIVE_TYPE_MAPPINGS: HashMap<SomeIpPrimitiveTypeDefinition, HashSet<Type>> = {
        HashMap::from([
            (SomeIpPrimitiveTypeDefinition::BOOLEAN, HashSet::from([Type::TYPE_BOOL])),
            (
                SomeIpPrimitiveTypeDefinition::U8,
                HashSet::from([
                    Type::TYPE_UINT32,
                    Type::TYPE_FIXED32,
                    Type::TYPE_UINT64,
                    Type::TYPE_FIXED64,
                    Type::TYPE_ENUM,
                ]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::U16,
                HashSet::from([
                    Type::TYPE_UINT32,
                    Type::TYPE_FIXED32,
                    Type::TYPE_UINT64,
                    Type::TYPE_FIXED64,
                    Type::TYPE_ENUM,
                ]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::U32,
                HashSet::from([
                    Type::TYPE_UINT32,
                    Type::TYPE_FIXED32,
                    Type::TYPE_UINT64,
                    Type::TYPE_FIXED64,
                ]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::U64,
                HashSet::from([Type::TYPE_UINT64, Type::TYPE_FIXED64]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::I8,
                HashSet::from([
                    Type::TYPE_INT32,
                    Type::TYPE_SINT32,
                    Type::TYPE_SFIXED32,
                    Type::TYPE_INT64,
                    Type::TYPE_SINT64,
                    Type::TYPE_SFIXED64,
                    Type::TYPE_ENUM,
                ]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::I16,
                HashSet::from([
                    Type::TYPE_INT32,
                    Type::TYPE_SINT32,
                    Type::TYPE_SFIXED32,
                    Type::TYPE_INT64,
                    Type::TYPE_SINT64,
                    Type::TYPE_SFIXED64,
                    Type::TYPE_ENUM,
                ]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::I32,
                HashSet::from([
                    Type::TYPE_INT32,
                    Type::TYPE_SINT32,
                    Type::TYPE_SFIXED32,
                    Type::TYPE_INT64,
                    Type::TYPE_SINT64,
                    Type::TYPE_SFIXED64,
                    Type::TYPE_ENUM,
                ]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::I64,
                HashSet::from([Type::TYPE_INT64, Type::TYPE_SINT64, Type::TYPE_SFIXED64]),
            ),
            (
                SomeIpPrimitiveTypeDefinition::F32,
                HashSet::from([Type::TYPE_FLOAT, Type::TYPE_DOUBLE]),
            ),
            (SomeIpPrimitiveTypeDefinition::F64, HashSet::from([Type::TYPE_DOUBLE])),
        ])
    };

    // This map provides the source of truth for the correct index of a valid primitive value
    // translation operation
    pub(crate) static ref TRANSLATION_OPERATION_INDICES: HashMap<(SomeIpPrimitiveTypeDefinition, Type), u8> = {
        let mut map = HashMap::new();
        let mut index = 0;
        let mut add = |s, p| {
            map.insert((s, p), index);
            index += 1;
        };
        add(SomeIpPrimitiveTypeDefinition::BOOLEAN, Type::TYPE_BOOL);
        add(SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT32);
        add(SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_ENUM);
        add(SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_FIXED32);
        add(SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT64);
        add(SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_FIXED64);
        add(SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_UINT32);
        add(SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_ENUM);
        add(SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_FIXED32);
        add(SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_UINT64);
        add(SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_FIXED64);
        add(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32);
        add(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_FIXED32);
        add(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT64);
        add(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_FIXED64);
        add(SomeIpPrimitiveTypeDefinition::U64, Type::TYPE_UINT64);
        add(SomeIpPrimitiveTypeDefinition::U64, Type::TYPE_FIXED64);
        add(SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SINT32);
        add(SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_INT32);
        add(SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_ENUM);
        add(SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SFIXED32);
        add(SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SINT64);
        add(SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_INT64);
        add(SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SFIXED64);
        add(SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_INT32);
        add(SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SINT32);
        add(SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_ENUM);
        add(SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SFIXED32);
        add(SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_INT64);
        add(SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SINT64);
        add(SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SFIXED64);
        add(SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_INT32);
        add(SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SINT32);
        add(SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_ENUM);
        add(SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SFIXED32);
        add(SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_INT64);
        add(SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SINT64);
        add(SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SFIXED64);
        add(SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_INT64);
        add(SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_SINT64);
        add(SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_SFIXED64);
        add(SomeIpPrimitiveTypeDefinition::F32, Type::TYPE_FLOAT);
        add(SomeIpPrimitiveTypeDefinition::F32, Type::TYPE_DOUBLE);
        add(SomeIpPrimitiveTypeDefinition::F64, Type::TYPE_DOUBLE);
        map
    };
}
