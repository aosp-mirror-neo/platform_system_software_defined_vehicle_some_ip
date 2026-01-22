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
//! Contains method for parsing byte data into SOME/IP structures

use byteorder::{BigEndian, LittleEndian};

use ::protobuf::reflect::MessageDescriptor;
use ::protobuf::MessageDyn;

use deserializer::SomeIpSerdeDeserializer;
pub use error::SomeIpParseError;
use protobuf::StructValueSeed as ProtoStructValueSeed;
use someip::StructValueSeed;
use someip_config::serde_config::ffi::SomeIpConfig;

use someip_config::serde_config::ffi::Endianness;

use someip_serde_reflect::{SomeIpStructType, SomeIpStructTypeDefinition};

use serde::de::DeserializeSeed;

mod common;
mod deserializer;
mod error;

mod protobuf;
mod someip;

/// Translate SOME/IP wire format to a SomeIpStructType
#[tracing::instrument(skip_all)]
pub fn get_someip_struct_from_bytes(
    input: &[u8],
    config: SomeIpConfig,
    type_definition: &SomeIpStructTypeDefinition,
) -> Result<SomeIpStructType, SomeIpParseError> {
    match config.endianness {
        Endianness::BigEndian => {
            let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(input);
            StructValueSeed::new(type_definition, config.endianness).deserialize(&mut deserializer)
        }
        Endianness::LittleEndian => {
            let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(input);
            StructValueSeed::new(type_definition, config.endianness).deserialize(&mut deserializer)
        }
        _ => Err(SomeIpParseError::Failure("Unknown endianness".to_string())),
    }
}

/// Translate SOME/IP wire format to a SomeIpStructType
#[tracing::instrument(skip_all)]
pub fn get_message_dyn_from_bytes(
    input: &[u8],
    config: SomeIpConfig,
    type_definition: &SomeIpStructTypeDefinition,
    proto_message_descriptor: &MessageDescriptor,
) -> Result<Box<dyn MessageDyn>, SomeIpParseError> {
    match config.endianness {
        Endianness::BigEndian => {
            let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(input);
            ProtoStructValueSeed::new(type_definition, proto_message_descriptor, config.endianness)
                .deserialize(&mut deserializer)
        }
        Endianness::LittleEndian => {
            let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(input);
            ProtoStructValueSeed::new(type_definition, proto_message_descriptor, config.endianness)
                .deserialize(&mut deserializer)
        }
        _ => Err(SomeIpParseError::Failure("Unknown endianness".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};
    use someip_serde_reflect::{
        SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
        SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
    };

    use crate::parser::get_message_dyn_from_bytes;
    use crate::parser::get_someip_struct_from_bytes;
    use protobuf::MessageFull;
    use someip_serde_test_protos::messages::ComplexTestMessage;
    use someip_serde_test_protos::messages::SimpleTestMessage;

    fn get_primitive(
        field_name: &str,
        someip_type: SomeIpPrimitiveTypeDefinition,
    ) -> SomeIpStructFieldTypeDefinition {
        SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: field_name.to_string(),
            field_type: SomeIpMessageTypeDefinition::PRIMITIVE(someip_type),
        }
    }

    fn get_primitive_array(
        field_name: &str,
        someip_type: SomeIpPrimitiveTypeDefinition,
    ) -> SomeIpStructFieldTypeDefinition {
        SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: field_name.to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(someip_type)),
            }),
        }
    }

    fn get_complex_struct_type_definition() -> SomeIpStructTypeDefinition {
        let struct_a = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "a".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("c", SomeIpPrimitiveTypeDefinition::U8),
                ]
                .to_vec(),
            }),
        };

        let struct_c = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "c".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::U8),
                ]
                .to_vec(),
            }),
        };

        let struct_hag = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "g".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            get_primitive("a", SomeIpPrimitiveTypeDefinition::U32),
                            get_primitive("b", SomeIpPrimitiveTypeDefinition::U32),
                            get_primitive("c", SomeIpPrimitiveTypeDefinition::U8),
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_ha = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "a".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("c", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("d", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("f", SomeIpPrimitiveTypeDefinition::U8),
                    struct_hag,
                ]
                .to_vec(),
            }),
        };

        let struct_hbc = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "c".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                            get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_hb = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "b".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,

                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                    struct_hbc,
                ]
                .to_vec(),
            }),
        };

        let struct_hc = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "c".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("c", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("d", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::F32),
                ]
                .to_vec(),
            }),
        };

        let struct_hd = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "d".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive_array("a", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                ]
                .to_vec(),
            }),
        };

        let struct_he = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "e".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive_array("a", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("c", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive_array("d", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("f", SomeIpPrimitiveTypeDefinition::F32),
                    get_primitive("g", SomeIpPrimitiveTypeDefinition::U8),
                ]
                .to_vec(),
            }),
        };

        let struct_hfb = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "b".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::OneByte,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                            get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_hf = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "f".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [get_primitive("a", SomeIpPrimitiveTypeDefinition::U8), struct_hfb]
                    .to_vec(),
            }),
        };

        let struct_hg = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "g".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    get_primitive("a", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive_array("b", SomeIpPrimitiveTypeDefinition::F32),
                ]
                .to_vec(),
            }),
        };

        let struct_hh = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "h".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [get_primitive("a", SomeIpPrimitiveTypeDefinition::F32)].to_vec(),
            }),
        };

        let struct_h = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "h".to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(SomeIpArrayTypeDefinition {
                fixed_length: None,
                length_field_size: LenFieldSize::TwoBytes,
                array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(
                    SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            struct_ha, struct_hb, struct_hc, struct_hd, struct_he, struct_hf,
                            struct_hg, struct_hh,
                        ]
                        .to_vec(),
                    },
                )),
            }),
        };

        let struct_data = SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: "a".to_string(),
            field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                name: "".to_string(),
                length_field_size: LenFieldSize::ZeroBytes,
                fields: [
                    struct_a,
                    get_primitive("b", SomeIpPrimitiveTypeDefinition::U8),
                    struct_c,
                    get_primitive("d", SomeIpPrimitiveTypeDefinition::U32),
                    get_primitive("e", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("f", SomeIpPrimitiveTypeDefinition::U8),
                    get_primitive("g", SomeIpPrimitiveTypeDefinition::U8),
                    struct_h,
                ]
                .to_vec(),
            }),
        };

        SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: [struct_data].to_vec(),
        }
    }

    #[test]
    fn deserialize_complex_struct() {
        let config = SomeIpConfig {
            endianness: Endianness::BigEndian,
            string_type: StringType::UTF_8,
            len_field_size: LenFieldSize::ZeroBytes,
        };

        let type_definition = get_complex_struct_type_definition();
        let buffers = get_complex_struct_buffers();

        for buffer in buffers.iter() {
            assert!(get_someip_struct_from_bytes(buffer, config, &type_definition).is_ok());
        }
    }

    /// Returns valid SOME/IP serialized buffers for the struct, as defined by
    /// the return value of `get_complex_struct_type_definition()`.
    /// These buffers can be created by instantiating a valid
    /// `someip_serde_test_protos::messages::ComplexTestMessage` proto message,
    /// then translating it toSOME/IP with the broker's
    /// `translate_from_protobuf_to_someip()` function and printing the result.
    fn get_complex_struct_buffers() -> Vec<Vec<u8>> {
        vec![
            vec![
                100, 101, 102, 10, 103, 3, 104, 105, 106, 0, 0, 0, 11, 12, 13, 14, 1, 46, 0, 0, 0,
                3, 0, 0, 0, 4, 0, 0, 0, 5, 6, 0, 7, 18, 0, 0, 0, 33, 0, 0, 0, 34, 35, 0, 0, 0, 43,
                0, 0, 0, 44, 45, 13, 14, 4, 33, 34, 43, 44, 64, 8, 63, 140, 204, 205, 63, 153, 153,
                154, 8, 63, 166, 102, 102, 63, 179, 51, 51, 63, 192, 0, 0, 63, 204, 204, 205, 8,
                65, 49, 153, 154, 65, 51, 51, 51, 8, 65, 52, 204, 205, 65, 54, 102, 102, 8, 65,
                168, 204, 205, 65, 169, 153, 154, 8, 65, 170, 102, 102, 65, 171, 51, 51, 8, 65,
                172, 0, 0, 65, 172, 204, 205, 8, 65, 173, 153, 154, 65, 174, 102, 102, 65, 175, 51,
                51, 65, 176, 0, 0, 15, 16, 4, 83, 84, 93, 94, 111, 8, 191, 140, 204, 205, 191, 153,
                153, 154, 66, 186, 4, 208, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 5, 0, 6, 18, 0, 0,
                0, 22, 0, 0, 0, 23, 24, 0, 0, 0, 32, 0, 0, 0, 33, 34, 2, 3, 4, 22, 23, 32, 33, 53,
                8, 0, 0, 0, 0, 61, 204, 204, 205, 8, 62, 76, 204, 205, 62, 153, 153, 154, 62, 204,
                204, 205, 63, 0, 0, 0, 8, 0, 0, 0, 0, 61, 204, 204, 205, 8, 62, 76, 204, 205, 62,
                153, 153, 154, 8, 65, 32, 0, 0, 65, 33, 153, 154, 8, 65, 35, 51, 51, 65, 36, 204,
                205, 8, 65, 38, 102, 102, 65, 40, 0, 0, 8, 65, 41, 153, 154, 65, 43, 51, 51, 65,
                44, 204, 205, 65, 62, 102, 102, 4, 5, 4, 72, 73, 82, 83, 0, 8, 128, 0, 0, 0, 189,
                204, 204, 205, 66, 165, 255, 33,
            ],
            vec![
                1, 1, 1, 1, 31, 31, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
                18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 0, 0, 0, 1, 1, 1, 0, 1, 41, 0,
                0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 108, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1,
                0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0,
                0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0,
                0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 20, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                1, 1, 1, 1, 1, 1, 1, 1, 1, 12, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 24, 63,
                128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0,
                0, 63, 128, 0, 0, 63, 128, 0, 0, 12, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0,
                12, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 8, 63, 128, 0, 0, 63, 128, 0, 0,
                12, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 8, 63, 128, 0, 0, 63, 128, 0, 0,
                12, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0, 1,
                1, 8, 1, 1, 1, 1, 1, 1, 1, 1, 1, 8, 63, 128, 0, 0, 63, 128, 0, 0, 63, 128, 0, 0,
            ],
        ]
    }

    #[test]
    fn deserialize_complex_struct_to_proto() {
        let config = SomeIpConfig {
            endianness: Endianness::BigEndian,
            string_type: StringType::UTF_8,
            len_field_size: LenFieldSize::ZeroBytes,
        };

        let type_definition = get_complex_struct_type_definition();
        let buffers = get_complex_struct_buffers();

        for buffer in buffers.iter() {
            assert!(get_message_dyn_from_bytes(
                buffer,
                config,
                &type_definition,
                &ComplexTestMessage::descriptor()
            )
            .is_ok());
        }
    }

    #[test]
    fn deserialize_simple_struct_to_proto() {
        let config = SomeIpConfig {
            endianness: Endianness::BigEndian,
            string_type: StringType::UTF_8,
            len_field_size: LenFieldSize::ZeroBytes,
        };

        let type_definition = SomeIpStructTypeDefinition {
            name: String::new(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "a".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U64,
                    ),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "b".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::F64,
                    ),
                },
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "c".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::U8,
                    ),
                },
            ],
        };
        let buffer: Vec<u8> =
            vec![0, 0, 0, 0, 0, 0, 233, 165, 64, 130, 140, 81, 235, 133, 30, 184, 11];

        assert!(get_message_dyn_from_bytes(
            &buffer,
            config,
            &type_definition,
            &SimpleTestMessage::descriptor()
        )
        .is_ok());
    }
}
