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

use protobuf::reflect::{FieldDescriptor, ReflectValueRef, RuntimeType};
use protobuf::MessageDyn;

use serde::ser::{Error, SerializeSeq};
use serde::Serialize;

use crate::serializer::get_byte_size;
use crate::serializer::primitive::PrimitiveValueSeed;
use crate::serializer::string::StringValueSeed;
use crate::serializer::StructValueSeed;
use someip_serde_reflect::{
    SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
};

pub(crate) struct ArrayValueSeed<'lt> {
    array_type_definition: &'lt SomeIpArrayTypeDefinition,
    proto_field_descriptor: &'lt FieldDescriptor,
    message: &'lt dyn MessageDyn,
}

impl<'lt> ArrayValueSeed<'lt> {
    pub(crate) fn new(
        array_type_definition: &'lt SomeIpArrayTypeDefinition,
        proto_field_descriptor: &'lt FieldDescriptor,
        message: &'lt dyn MessageDyn,
    ) -> Self {
        Self { array_type_definition, proto_field_descriptor, message }
    }
}

impl Serialize for ArrayValueSeed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let optional_size = if self.array_type_definition.fixed_length.is_some() {
            None
        } else {
            Some(
                get_byte_size(&self.array_type_definition.length_field_size)
                    .map_err(|e| S::Error::custom(e.to_string()))?,
            )
        };

        let mut seq = serializer.serialize_seq(optional_size)?;

        match &*self.array_type_definition.array_type {
            SomeIpMessageTypeDefinition::PRIMITIVE(primitive_type_definition) => {
                if self.proto_field_descriptor.is_singular() {
                    // this can only be a bytes array
                    if let ReflectValueRef::Bytes(bytes_value) =
                        self.proto_field_descriptor.get_singular_field_or_default(self.message)
                    {
                        if *primitive_type_definition != SomeIpPrimitiveTypeDefinition::U8 {
                            return Err(S::Error::custom("Expected to serialize a protobuf bytes field to a SOME/IP U8 array"));
                        }
                        seq.serialize_element(&BytesValueSeed::new(bytes_value))?;
                    } else {
                        return Err(S::Error::custom(
                            "Expected a protobuf bytes field for a SOME/IP primitive array",
                        ));
                    }
                } else if self.proto_field_descriptor.is_repeated() {
                    let repeated_ref = self.proto_field_descriptor.get_repeated(self.message);
                    for array_element in repeated_ref {
                        seq.serialize_element(&PrimitiveValueSeed::new(
                            primitive_type_definition,
                            array_element,
                        ))?;
                    }
                } else {
                    return Err(S::Error::custom("Expected a singular or repeated protobuf field for SOME/IP primitive array"));
                }
            }
            SomeIpMessageTypeDefinition::STRING(string_type_definition) => {
                if self.proto_field_descriptor.is_repeated() {
                    let repeated_ref = self.proto_field_descriptor.get_repeated(self.message);
                    for array_element in repeated_ref {
                        seq.serialize_element(&StringValueSeed::new(
                            string_type_definition,
                            array_element,
                        ))?;
                    }
                } else {
                    return Err(S::Error::custom(
                        "Expected a repeated protobuf field for SOME/IP string array",
                    ));
                }
            }
            SomeIpMessageTypeDefinition::ARRAY(array_type_definition) => {
                // A SOME/IP 2d array can be represented as a `repeated bytes` field in protobuf
                if self.proto_field_descriptor.is_repeated() {
                    let repeated_ref = self.proto_field_descriptor.get_repeated(self.message);
                    if repeated_ref.element_type() != RuntimeType::VecU8 {
                        return Err(S::Error::custom(
                            "Expected a repeated bytes field for SOME/IP 2d array",
                        ));
                    }
                    for array_element in repeated_ref {
                        seq.serialize_element(&RepeatedBytesSeed::new(
                            array_type_definition,
                            array_element,
                        ))?;
                    }
                } else {
                    return Err(S::Error::custom(
                        "Expected a repeated protobuf field for SOME/IP 2d array",
                    ));
                }
            }
            SomeIpMessageTypeDefinition::STRUCT(struct_type_definition) => {
                if self.proto_field_descriptor.is_repeated() {
                    let repeated_ref = self.proto_field_descriptor.get_repeated(self.message);
                    let RuntimeType::Message(descriptor) = repeated_ref.element_type() else {
                        return Err(S::Error::custom(
                            "Expected a proto message type for SOME/IP struct",
                        ));
                    };
                    for array_element in &repeated_ref {
                        let ReflectValueRef::Message(msg) = array_element else {
                            return Err(S::Error::custom(
                                "Expected a proto message type for SOME/IP array",
                            ));
                        };
                        seq.serialize_element(&StructValueSeed::new(
                            struct_type_definition,
                            &descriptor,
                            &*msg,
                        ))?;
                    }
                } else {
                    return Err(S::Error::custom(
                        "Expected a repeated protobuf field for SOME/IP struct array",
                    ));
                }
            }
        }
        seq.end()
    }
}

struct BytesValueSeed<'lt> {
    bytes_value: &'lt [u8],
}

impl<'lt> BytesValueSeed<'lt> {
    fn new(bytes_value: &'lt [u8]) -> Self {
        BytesValueSeed { bytes_value }
    }
}

impl Serialize for BytesValueSeed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.bytes_value)
    }
}

struct RepeatedBytesSeed<'lt> {
    array_type_definition: &'lt SomeIpArrayTypeDefinition,
    proto_value: ReflectValueRef<'lt>,
}

impl<'lt> RepeatedBytesSeed<'lt> {
    fn new(
        array_type_definition: &'lt SomeIpArrayTypeDefinition,
        proto_value: ReflectValueRef<'lt>,
    ) -> Self {
        RepeatedBytesSeed { array_type_definition, proto_value }
    }
}

impl Serialize for RepeatedBytesSeed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let optional_size = if self.array_type_definition.fixed_length.is_some() {
            None
        } else {
            Some(
                get_byte_size(&self.array_type_definition.length_field_size)
                    .map_err(|e| S::Error::custom(e.to_string()))?,
            )
        };

        let mut seq = serializer.serialize_seq(optional_size)?;
        let ReflectValueRef::Bytes(bytes_value) = self.proto_value else {
            return Err(S::Error::custom("Internal error, expected a proto bytes array"));
        };
        seq.serialize_element(&BytesValueSeed::new(bytes_value))?;
        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serializer::string::{BOM_UTF16_BE, BOM_UTF8};
    use crate::serializer::SomeIpSerdeSerializer;
    use byteorder::BigEndian;
    use protobuf::MessageDyn;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, SomeIpConfig, StringType};
    use someip_serde_reflect::{
        SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
        SomeIpStringTypeDefinition, SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
    };
    use someip_serde_test_protos::messages::{
        ArrayMessage, ArrayOfStructsMessage, Point, RepeatedStringMessage, SimpleTestMessage,
        TwoDArrayMessage,
    };

    const DEFAULT_SOMEIP_CONFIG: SomeIpConfig = SomeIpConfig {
        endianness: Endianness::BigEndian,
        string_type: StringType::UTF_8,
        len_field_size: LenFieldSize::OneByte, // Default for serializer in these tests
    };

    #[test]
    fn test_serialize_repeated_i32_as_i32_array_dynamic_length() {
        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I32,
            )),
            length_field_size: LenFieldSize::TwoBytes, // Array's own length field is 2 bytes wide
            fixed_length: None,
        };

        let mut msg = ArrayMessage::new();
        let test_values: Vec<i32> = vec![1000, 2000];
        msg.field = test_values.clone();

        let msg_dyn: &dyn MessageDyn = &msg;
        let field_descriptor = msg.descriptor_dyn().field_by_name("field").unwrap();

        let seed = ArrayValueSeed::new(&array_type_def, &field_descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();

        let total_element_bytes = (test_values.len() * std::mem::size_of::<i32>()) as u16; // 2 * 4 = 8
        expected_bytes.extend_from_slice(&total_element_bytes.to_be_bytes()); // [0x00, 0x08] for BigEndian
        for val in &test_values {
            expected_bytes.extend_from_slice(&val.to_be_bytes());
        }
        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }

    #[test]
    fn test_serialize_fixed_length_repeated_i16_array() {
        let fixed_len_val = 2;
        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::I16,
            )),
            length_field_size: LenFieldSize::OneByte, // Irrelevant for fixed_length
            fixed_length: Some(fixed_len_val),
        };

        let mut msg = ArrayMessage::new();
        let test_values: Vec<i32> = vec![10, 20]; // Protobuf field is int32
        msg.field = test_values.clone(); // PrimitiveValueSeed will convert i32 to i16

        let msg_dyn: &dyn MessageDyn = &msg;
        let field_descriptor = msg.descriptor_dyn().field_by_name("field").unwrap();

        let seed = ArrayValueSeed::new(&array_type_def, &field_descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();
        // No length prefix for fixed-length array.
        for val_i32 in &test_values {
            expected_bytes.extend_from_slice(&(i16::try_from(*val_i32).unwrap()).to_be_bytes());
        }
        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }

    #[test]
    fn test_serialize_error_primitive_array_expects_singular_bytes_field_gets_singular_u32_field() {
        // This tests the path: is_singular() is true, but it's not ReflectValueRef::Bytes
        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                // This implies singular bytes path
                SomeIpPrimitiveTypeDefinition::U8,
            )),
            length_field_size: LenFieldSize::OneByte,
            fixed_length: None,
        };

        let mut msg = SimpleTestMessage::new();
        msg.c = 123; // Singular uint32, but not ReflectValueRef::Bytes

        let msg_dyn: &dyn MessageDyn = &msg;
        let field_descriptor = msg.descriptor_dyn().field_by_name("c").unwrap();

        let seed = ArrayValueSeed::new(&array_type_def, &field_descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        let result = seed.serialize(&mut serializer);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unrecoverable parsing error: Expected a protobuf bytes field for a SOME/IP primitive array");
    }

    #[test]
    fn test_serialize_repeated_string_array_dynamic_length_utf8() {
        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                length_field_size: LenFieldSize::OneByte, // String's own length field
                bom: StringType::UTF_8,
                fixed_length: None,
            })),
            length_field_size: LenFieldSize::TwoBytes, // Array's own length field
            fixed_length: None,
        };

        let mut msg = RepeatedStringMessage::new();
        let test_values: Vec<String> = vec!["hello".to_string(), "world".to_string()];
        msg.field = test_values.clone();

        let msg_dyn: &dyn MessageDyn = &msg;
        let field_descriptor = msg.descriptor_dyn().field_by_name("field").unwrap();

        let seed = ArrayValueSeed::new(&array_type_def, &field_descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();
        let mut total_elements_byte_length: u16 = 0;

        let mut string_bytes_data = Vec::new();
        for s_val in &test_values {
            let mut current_string_bytes = Vec::new();
            current_string_bytes.extend_from_slice(BOM_UTF8);
            current_string_bytes.extend_from_slice(s_val.as_bytes());
            current_string_bytes.push(0x00); // Null terminator

            string_bytes_data.push(current_string_bytes.len() as u8); // String length field (1 byte)
            string_bytes_data.extend_from_slice(&current_string_bytes);
            total_elements_byte_length += (current_string_bytes.len() + 1) as u16;
            // +1 for string length field
        }

        expected_bytes.extend_from_slice(&total_elements_byte_length.to_be_bytes()); // Array length
        expected_bytes.extend_from_slice(&string_bytes_data);

        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }

    #[test]
    fn test_serialize_repeated_string_array_fixed_length_utf16be() {
        // Each string element itself is fixed length. The array contains a fixed number of such elements.
        let string_fixed_len = 10; // BOM (2) + "hi" (2*2=4) + NULL (2) + padding (2) = 10
        let array_fixed_num_elements = 2;

        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::STRING(SomeIpStringTypeDefinition {
                length_field_size: LenFieldSize::ZeroBytes, // Not used for fixed length strings
                bom: StringType::UTF_16_BE,
                fixed_length: Some(string_fixed_len),
            })),
            length_field_size: LenFieldSize::ZeroBytes, // Not used for fixed length arrays
            fixed_length: Some(array_fixed_num_elements), // Fixed number of elements
        };

        let mut msg = RepeatedStringMessage::new();
        let test_values: Vec<String> = vec!["hi".to_string(), "go".to_string()];
        msg.field = test_values.clone();

        let msg_dyn: &dyn MessageDyn = &msg;
        let field_descriptor = msg.descriptor_dyn().field_by_name("field").unwrap();

        let seed = ArrayValueSeed::new(&array_type_def, &field_descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();

        for s_val in &test_values {
            let mut current_string_bytes = Vec::new();
            current_string_bytes.extend_from_slice(BOM_UTF16_BE);
            for char_utf16 in s_val.encode_utf16() {
                current_string_bytes.extend_from_slice(&char_utf16.to_be_bytes());
            }
            current_string_bytes.extend_from_slice(&0u16.to_be_bytes()); // Null terminator

            if current_string_bytes.len() < string_fixed_len {
                current_string_bytes.resize(string_fixed_len, 0x00); // Pad
            }
            expected_bytes.extend_from_slice(&current_string_bytes);
        }
        let serialized_bytes = serializer.get_serialized_bytes();

        assert_eq!(serialized_bytes.len(), array_fixed_num_elements * string_fixed_len);
        assert_eq!(serialized_bytes, expected_bytes);
    }

    #[test]
    fn test_serialize_2d_array_u8_dynamic_outer_fixed_inner() {
        // Inner array: fixed length (3) array of U8
        let inner_array_fixed_len = 3;
        let inner_array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                SomeIpPrimitiveTypeDefinition::U8,
            )),
            length_field_size: LenFieldSize::ZeroBytes, // Not used for fixed length inner array
            fixed_length: Some(inner_array_fixed_len),
        };

        let outer_array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::ARRAY(inner_array_type_def.clone())),
            length_field_size: LenFieldSize::TwoBytes, // Outer array's own length field (for total bytes of inner arrays' payload)
            fixed_length: None,                        // Dynamic number of inner arrays
        };

        let mut msg = TwoDArrayMessage::new();
        let row1: Vec<u8> = vec![1, 2, 3];
        let row2: Vec<u8> = vec![4, 5, 6];
        // Ensure rows match inner_array_fixed_len for this test's assumption
        assert_eq!(row1.len(), inner_array_fixed_len);
        assert_eq!(row2.len(), inner_array_fixed_len);
        msg.matrix = vec![row1.clone(), row2.clone()]; // Protobuf `repeated bytes`

        let msg_dyn: &dyn MessageDyn = &msg;
        let field_descriptor = msg.descriptor_dyn().field_by_name("matrix").unwrap();

        let seed = ArrayValueSeed::new(&outer_array_type_def, &field_descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();
        // Total length of all inner array data (payload only, as inner arrays are fixed length)
        let total_elements_payload_byte_length: u16 =
            (msg.matrix.len() * inner_array_fixed_len) as u16;
        expected_bytes.extend_from_slice(&total_elements_payload_byte_length.to_be_bytes()); // Outer array length

        for r_val_bytes in &msg.matrix {
            // Each r_val_bytes is a Vec<u8> from the repeated bytes field.
            // Since inner_array_type_def is fixed_length, RepeatedBytesSeed will not add a length prefix for these bytes.
            expected_bytes.extend_from_slice(r_val_bytes);
        }
        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }

    #[test]
    fn test_serialize_array_of_structs_dynamic_length() {
        // Define the SOME/IP struct type for Point
        let point_struct_type_def = SomeIpStructTypeDefinition {
            name: "Point".to_string(),
            fields: vec![
                SomeIpStructFieldTypeDefinition {
                    field_name: "x".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::I32,
                    ),
                    data_tag: None,
                },
                SomeIpStructFieldTypeDefinition {
                    field_name: "y".to_string(),
                    field_type: SomeIpMessageTypeDefinition::PRIMITIVE(
                        SomeIpPrimitiveTypeDefinition::I32,
                    ),
                    data_tag: None,
                },
            ],
            length_field_size: LenFieldSize::ZeroBytes, // Structs themselves don't have a leading length field here
        };

        // Define the SOME/IP array type for repeated Point
        let array_type_def = SomeIpArrayTypeDefinition {
            array_type: Box::new(SomeIpMessageTypeDefinition::STRUCT(point_struct_type_def)),
            length_field_size: LenFieldSize::TwoBytes, // Array's own length field is 2 bytes wide
            fixed_length: None,
        };

        // Create and populate the protobuf message
        let mut msg = ArrayOfStructsMessage::new();
        let mut point1 = Point::new();
        point1.x = 10;
        point1.y = 20;
        let mut point2 = Point::new();
        point2.x = 30;
        point2.y = 40;
        msg.points = vec![point1.clone(), point2.clone()];

        let msg_dyn: &dyn MessageDyn = &msg;
        let field_descriptor = msg.descriptor_dyn().field_by_name("points").unwrap();

        let seed = ArrayValueSeed::new(&array_type_def, &field_descriptor, msg_dyn);
        let mut serializer = SomeIpSerdeSerializer::<BigEndian>::new(DEFAULT_SOMEIP_CONFIG);
        seed.serialize(&mut serializer).expect("Serialization should succeed");

        let mut expected_bytes = Vec::new();

        // Each Point is x (i32) + y (i32) = 4 + 4 = 8 bytes. Two points = 16 bytes.
        let total_elements_byte_length: u16 =
            (msg.points.len() * (std::mem::size_of::<i32>() * 2)) as u16;
        expected_bytes.extend_from_slice(&total_elements_byte_length.to_be_bytes()); // Array length [0x00, 0x10]

        for p_val in &msg.points {
            expected_bytes.extend_from_slice(&p_val.x.to_be_bytes());
            expected_bytes.extend_from_slice(&p_val.y.to_be_bytes());
        }
        assert_eq!(serializer.get_serialized_bytes(), expected_bytes);
    }
}
