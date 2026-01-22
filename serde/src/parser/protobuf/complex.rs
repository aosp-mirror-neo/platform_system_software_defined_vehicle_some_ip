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

use crate::parser::common::LengthFieldDeserializeSeed;
use crate::parser::protobuf::array::ArrayValueDeserializeSeed;
use crate::parser::protobuf::primitive::PrimitiveValueSeed;
use crate::parser::protobuf::string::StringValueSeed;
use protobuf::reflect::{FieldDescriptor, MessageDescriptor, ReflectValueBox, RuntimeType};
use protobuf::MessageDyn;
use serde::de::{DeserializeSeed, Error, SeqAccess, Visitor};
use serde::Deserializer;
use someip_config::serde_config::ffi::{Endianness, LenFieldSize};
use someip_serde_reflect::{
    SomeIpMessageTypeDefinition, SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
};

pub(crate) struct StructValueSeed<'lt> {
    someip_struct_definition: &'lt SomeIpStructTypeDefinition,
    proto_message_descriptor: &'lt MessageDescriptor,
    endianness: Endianness,
}

impl<'lt> StructValueSeed<'lt> {
    pub(crate) fn new(
        someip_struct_definition: &'lt SomeIpStructTypeDefinition,
        proto_message_descriptor: &'lt MessageDescriptor,
        endianness: Endianness,
    ) -> Self {
        StructValueSeed { someip_struct_definition, proto_message_descriptor, endianness }
    }
}

impl<'de> DeserializeSeed<'de> for StructValueSeed<'_> {
    type Value = Box<dyn MessageDyn>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StructVisitor<'lt> {
            someip_struct_definition: &'lt SomeIpStructTypeDefinition,
            proto_message_descriptor: &'lt MessageDescriptor,
            endianness: Endianness,
        }

        impl<'lt> StructVisitor<'lt> {
            fn new(
                someip_struct_definition: &'lt SomeIpStructTypeDefinition,
                proto_message_descriptor: &'lt MessageDescriptor,
                endianness: Endianness,
            ) -> Self {
                Self { someip_struct_definition, proto_message_descriptor, endianness }
            }
        }

        impl<'de> Visitor<'de> for StructVisitor<'_> {
            type Value = Box<dyn MessageDyn>;

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP struct type")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut message = self.proto_message_descriptor.new_instance();
                let struct_size = match self.someip_struct_definition.length_field_size {
                    LenFieldSize::ZeroBytes => None, // OK to have no length field for structs
                    _ => {
                        let Some(size) = seq.next_element_seed(LengthFieldDeserializeSeed::new(
                            &self.someip_struct_definition.length_field_size,
                        ))?
                        else {
                            return Err(A::Error::custom("Missing SOME/IP struct size"));
                        };
                        Some(size)
                    }
                };
                match struct_size {
                    Some(size) => {
                        seq.next_element_seed(StructWithLengthValueSeed::new(
                            self.someip_struct_definition,
                            self.proto_message_descriptor,
                            self.endianness,
                            &mut message,
                            size,
                        ))?;
                        Ok(message)
                    }
                    None => {
                        for someip_field in &self.someip_struct_definition.fields {
                            let Some(proto_field_descriptor) = self
                                .proto_message_descriptor
                                .field_by_name(&someip_field.field_name)
                            else {
                                return Err(A::Error::custom(format!(
                                    "Could not find matching protobuf field with name {}",
                                    &someip_field.field_name,
                                )));
                            };
                            let Some(()) =
                                seq.next_element_seed(StructFieldDeserializeSeed::new(
                                    someip_field,
                                    &proto_field_descriptor,
                                    &mut message,
                                    self.endianness,
                                ))?
                            else {
                                return Err(A::Error::custom(
                                    "Expected more protobuf message fields",
                                ));
                            };
                        }
                        Ok(message)
                    }
                }
            }
        }
        deserializer.deserialize_seq(StructVisitor::new(
            self.someip_struct_definition,
            self.proto_message_descriptor,
            self.endianness,
        ))
    }
}

struct StructWithLengthValueSeed<'lt> {
    someip_struct_definition: &'lt SomeIpStructTypeDefinition,
    proto_message_descriptor: &'lt MessageDescriptor,
    endianness: Endianness,
    message: &'lt mut Box<dyn MessageDyn>,
    struct_size_in_bytes: usize,
}

impl<'lt> StructWithLengthValueSeed<'lt> {
    fn new(
        someip_struct_definition: &'lt SomeIpStructTypeDefinition,
        proto_message_descriptor: &'lt MessageDescriptor,
        endianness: Endianness,
        message: &'lt mut Box<dyn MessageDyn>,
        struct_size_in_bytes: usize,
    ) -> Self {
        StructWithLengthValueSeed {
            someip_struct_definition,
            proto_message_descriptor,
            endianness,
            message,
            struct_size_in_bytes,
        }
    }
}

impl<'de> DeserializeSeed<'de> for StructWithLengthValueSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StructWithLengthVisitor<'lt> {
            someip_struct_definition: &'lt SomeIpStructTypeDefinition,
            proto_message_descriptor: &'lt MessageDescriptor,
            endianness: Endianness,
            message: &'lt mut Box<dyn MessageDyn>,
        }

        impl<'lt> StructWithLengthVisitor<'lt> {
            fn new(
                someip_struct_definition: &'lt SomeIpStructTypeDefinition,
                proto_message_descriptor: &'lt MessageDescriptor,
                endianness: Endianness,
                message: &'lt mut Box<dyn MessageDyn>,
            ) -> Self {
                Self { someip_struct_definition, proto_message_descriptor, endianness, message }
            }
        }

        impl<'de> Visitor<'de> for StructWithLengthVisitor<'_> {
            type Value = ();

            fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "SOME/IP struct type")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                for someip_field in &self.someip_struct_definition.fields {
                    let Some(proto_field_descriptor) =
                        self.proto_message_descriptor.field_by_name(&someip_field.field_name)
                    else {
                        return Err(A::Error::custom(format!(
                            "Could not find matching protobuf field with name {}",
                            &someip_field.field_name,
                        )));
                    };
                    if seq
                        .next_element_seed(StructFieldDeserializeSeed::new(
                            someip_field,
                            &proto_field_descriptor,
                            self.message,
                            self.endianness,
                        ))?
                        .is_none()
                    {
                        break;
                    }
                }
                Ok(())
            }
        }
        deserializer.deserialize_tuple_struct(
            "",
            self.struct_size_in_bytes,
            StructWithLengthVisitor::new(
                self.someip_struct_definition,
                self.proto_message_descriptor,
                self.endianness,
                self.message,
            ),
        )
    }
}

struct StructFieldDeserializeSeed<'lt> {
    someip_field_type_definition: &'lt SomeIpStructFieldTypeDefinition,
    proto_field_descriptor: &'lt FieldDescriptor,
    message: &'lt mut Box<dyn MessageDyn>,
    endianness: Endianness,
}

impl<'lt> StructFieldDeserializeSeed<'lt> {
    fn new(
        someip_field_type_definition: &'lt SomeIpStructFieldTypeDefinition,
        proto_field_descriptor: &'lt FieldDescriptor,
        message: &'lt mut Box<dyn MessageDyn>,
        endianness: Endianness,
    ) -> Self {
        Self { someip_field_type_definition, proto_field_descriptor, message, endianness }
    }
}

impl<'de> DeserializeSeed<'de> for StructFieldDeserializeSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        match &self.someip_field_type_definition.field_type {
            SomeIpMessageTypeDefinition::PRIMITIVE(primitive_type_definition) => {
                if self.proto_field_descriptor.is_singular() {
                    let primitive_value = PrimitiveValueSeed::new(
                        primitive_type_definition,
                        &self.proto_field_descriptor.singular_runtime_type(),
                    )
                    .deserialize(deserializer)?;

                    self.proto_field_descriptor
                        .set_singular_field(&mut **self.message, primitive_value);
                } else {
                    return Err(D::Error::custom("Unexpected array"));
                }
            }
            SomeIpMessageTypeDefinition::STRING(string_type_definition) => {
                if self.proto_field_descriptor.is_singular() {
                    let string_value = StringValueSeed::new(
                        string_type_definition,
                        &self.proto_field_descriptor.singular_runtime_type(),
                    )
                    .deserialize(deserializer)?;

                    self.proto_field_descriptor
                        .set_singular_field(&mut **self.message, string_value);
                } else {
                    return Err(D::Error::custom("Unexpected array"));
                }
            }
            SomeIpMessageTypeDefinition::ARRAY(array_type_definition) => {
                ArrayValueDeserializeSeed::new(
                    array_type_definition,
                    self.proto_field_descriptor,
                    self.message,
                    self.endianness,
                )
                .deserialize(deserializer)?;
            }
            SomeIpMessageTypeDefinition::STRUCT(struct_type_definition) => {
                if self.proto_field_descriptor.is_singular() {
                    let RuntimeType::Message(msg) =
                        self.proto_field_descriptor.singular_runtime_type()
                    else {
                        return Err(D::Error::custom("Expected a protobuf message"));
                    };
                    let struct_value =
                        StructValueSeed::new(struct_type_definition, &msg, self.endianness)
                            .deserialize(deserializer)?;

                    self.proto_field_descriptor.set_singular_field(
                        &mut **self.message,
                        ReflectValueBox::Message(struct_value),
                    );
                } else {
                    return Err(D::Error::custom("Unexpected array"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{BigEndian, LittleEndian};
    use protobuf::reflect::{ReflectValueRef, RuntimeType};
    use protobuf::MessageFull;
    use serde::de::DeserializeSeed;
    use someip_config::serde_config::ffi::{Endianness, LenFieldSize, StringType};
    use someip_serde_reflect::{
        SomeIpArrayTypeDefinition, SomeIpMessageTypeDefinition, SomeIpPrimitiveTypeDefinition,
        SomeIpStringTypeDefinition, SomeIpStructFieldTypeDefinition, SomeIpStructTypeDefinition,
    };
    use someip_serde_test_protos::messages::{SimpleTestMessage, TestStruct};

    use crate::parser::deserializer::SomeIpSerdeDeserializer;
    use crate::parser::protobuf::complex::StructValueSeed;

    fn get_primitive_field_definition(
        field_name: &str,
        someip_type: SomeIpPrimitiveTypeDefinition,
    ) -> SomeIpStructFieldTypeDefinition {
        SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: field_name.to_string(),
            field_type: SomeIpMessageTypeDefinition::PRIMITIVE(someip_type),
        }
    }

    fn get_string_field_definition(
        field_name: &str,
        someip_type: SomeIpStringTypeDefinition,
    ) -> SomeIpStructFieldTypeDefinition {
        SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: field_name.to_string(),
            field_type: SomeIpMessageTypeDefinition::STRING(someip_type),
        }
    }

    fn get_array_field_definition(
        field_name: &str,
        someip_type: SomeIpArrayTypeDefinition,
    ) -> SomeIpStructFieldTypeDefinition {
        SomeIpStructFieldTypeDefinition {
            data_tag: None,
            field_name: field_name.to_string(),
            field_type: SomeIpMessageTypeDefinition::ARRAY(someip_type),
        }
    }

    #[test]
    fn test_deserialize_simple_struct_le() {
        let struct_definition = SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: [
                get_primitive_field_definition("a", SomeIpPrimitiveTypeDefinition::U64),
                get_primitive_field_definition("b", SomeIpPrimitiveTypeDefinition::F64),
                get_primitive_field_definition("c", SomeIpPrimitiveTypeDefinition::U32),
            ]
            .to_vec(),
        };

        let u64_val = 54u64;
        let f64_val = 8.23f64;
        let u32_val = 12u32;

        let mut struct_payload = Vec::new();
        struct_payload.extend_from_slice(&u64_val.to_le_bytes());
        struct_payload.extend_from_slice(&f64_val.to_le_bytes());
        struct_payload.extend_from_slice(&u32_val.to_le_bytes());

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&struct_payload);
        let message_descriptor = SimpleTestMessage::descriptor();

        let struct_value_seed =
            StructValueSeed::new(&struct_definition, &message_descriptor, Endianness::LittleEndian);
        let deserialized_msg =
            struct_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let u64_field_descriptor =
            message_descriptor.field_by_name("a").expect("Field 'a' must be available");
        let f64_field_descriptor =
            message_descriptor.field_by_name("b").expect("Field 'b' must be available");
        let u32_field_descriptor =
            message_descriptor.field_by_name("c").expect("Field 'c' must be available");

        let u64_field = u64_field_descriptor.get_singular_field_or_default(&*deserialized_msg);
        assert_eq!(u64_field, ReflectValueRef::U64(u64_val));

        let f64_field = f64_field_descriptor.get_singular_field_or_default(&*deserialized_msg);
        assert_eq!(f64_field, ReflectValueRef::F64(f64_val));

        let u32_field = u32_field_descriptor.get_singular_field_or_default(&*deserialized_msg);
        assert_eq!(u32_field, ReflectValueRef::U32(u32_val));
    }

    #[test]
    fn test_deserialize_simple_struct_with_length_le() {
        let struct_definition = SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::OneByte,
            fields: [
                get_primitive_field_definition("a", SomeIpPrimitiveTypeDefinition::U64),
                get_primitive_field_definition("b", SomeIpPrimitiveTypeDefinition::F64),
                get_primitive_field_definition("c", SomeIpPrimitiveTypeDefinition::U32),
            ]
            .to_vec(),
        };

        let u64_val = 54u64;
        let f64_val = 8.23f64;
        let u32_val = 12u32;

        let mut struct_payload = Vec::new();
        struct_payload.extend_from_slice(&u64_val.to_le_bytes());
        struct_payload.extend_from_slice(&f64_val.to_le_bytes());
        struct_payload.extend_from_slice(&u32_val.to_le_bytes());

        let mut payload = Vec::new();
        payload.push(struct_payload.len() as u8);
        payload.extend_from_slice(&struct_payload);

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&payload);
        let message_descriptor = SimpleTestMessage::descriptor();

        let struct_value_seed =
            StructValueSeed::new(&struct_definition, &message_descriptor, Endianness::LittleEndian);
        let deserialized_msg =
            struct_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let u64_field_descriptor =
            message_descriptor.field_by_name("a").expect("Field 'a' must be available");
        let f64_field_descriptor =
            message_descriptor.field_by_name("b").expect("Field 'b' must be available");
        let u32_field_descriptor =
            message_descriptor.field_by_name("c").expect("Field 'c' must be available");

        let u64_field = u64_field_descriptor.get_singular_field_or_default(&*deserialized_msg);
        assert_eq!(u64_field, ReflectValueRef::U64(u64_val));

        let f64_field = f64_field_descriptor.get_singular_field_or_default(&*deserialized_msg);
        assert_eq!(f64_field, ReflectValueRef::F64(f64_val));

        let u32_field = u32_field_descriptor.get_singular_field_or_default(&*deserialized_msg);
        assert_eq!(u32_field, ReflectValueRef::U32(u32_val));
    }

    #[test]
    fn test_deserialize_simple_struct_be() {
        let struct_definition = SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: [
                get_primitive_field_definition("a", SomeIpPrimitiveTypeDefinition::U64),
                get_primitive_field_definition("b", SomeIpPrimitiveTypeDefinition::F64),
                get_primitive_field_definition("c", SomeIpPrimitiveTypeDefinition::U32),
            ]
            .to_vec(),
        };

        let u64_val = 0x123456789ABCDEF0u64;
        let f64_val = -123.456f64;
        let u32_val = 0xAABBCCDDu32;

        let mut struct_payload = Vec::new();
        struct_payload.extend_from_slice(&u64_val.to_be_bytes());
        struct_payload.extend_from_slice(&f64_val.to_be_bytes());
        struct_payload.extend_from_slice(&u32_val.to_be_bytes());

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&struct_payload);
        let message_descriptor = SimpleTestMessage::descriptor();

        let struct_value_seed =
            StructValueSeed::new(&struct_definition, &message_descriptor, Endianness::BigEndian);
        let deserialized_msg =
            struct_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let u64_field_descriptor =
            message_descriptor.field_by_name("a").expect("Field 'a' must be available");
        let f64_field_descriptor =
            message_descriptor.field_by_name("b").expect("Field 'b' must be available");
        let u32_field_descriptor =
            message_descriptor.field_by_name("c").expect("Field 'c' must be available");

        assert_eq!(
            u64_field_descriptor.get_singular_field_or_default(&*deserialized_msg),
            ReflectValueRef::U64(u64_val)
        );
        assert_eq!(
            f64_field_descriptor.get_singular_field_or_default(&*deserialized_msg),
            ReflectValueRef::F64(f64_val)
        );
        assert_eq!(
            u32_field_descriptor.get_singular_field_or_default(&*deserialized_msg),
            ReflectValueRef::U32(u32_val)
        );
    }

    #[test]
    fn test_deserialize_simple_struct_with_length_be() {
        let struct_definition = SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::OneByte, // Struct has a 1-byte length field
            fields: [
                get_primitive_field_definition("a", SomeIpPrimitiveTypeDefinition::U64),
                get_primitive_field_definition("c", SomeIpPrimitiveTypeDefinition::U32), // Field b is skipped
            ]
            .to_vec(),
        };

        let u64_val = 0x1122334455667788u64;
        let u32_val = 0x99AABBCCu32;

        let mut struct_data_payload = Vec::new();
        struct_data_payload.extend_from_slice(&u64_val.to_be_bytes());
        struct_data_payload.extend_from_slice(&u32_val.to_be_bytes());

        let mut payload_with_len = vec![struct_data_payload.len() as u8];
        payload_with_len.extend_from_slice(&struct_data_payload);

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&payload_with_len);
        let message_descriptor = SimpleTestMessage::descriptor();

        let struct_value_seed =
            StructValueSeed::new(&struct_definition, &message_descriptor, Endianness::BigEndian);
        let deserialized_msg =
            struct_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let u64_field_descriptor =
            message_descriptor.field_by_name("a").expect("Field 'a' must be available");
        let f64_field_descriptor =
            message_descriptor.field_by_name("b").expect("Field 'b' must be available");
        let u32_field_descriptor =
            message_descriptor.field_by_name("c").expect("Field 'c' must be available");

        assert_eq!(
            u64_field_descriptor.get_singular_field_or_default(&*deserialized_msg),
            ReflectValueRef::U64(u64_val)
        );
        assert_eq!(
            u32_field_descriptor.get_singular_field_or_default(&*deserialized_msg),
            ReflectValueRef::U32(u32_val)
        );
        // Field "b" should have its default value as it was not in the SOME/IP definition for this test
        assert_eq!(
            f64_field_descriptor.get_singular_field_or_default(&*deserialized_msg),
            ReflectValueRef::F64(0.0f64)
        );
    }

    #[test]
    fn test_deserialize_complex_struct_be() {
        let struct_definition = SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: [
                get_string_field_definition(
                    "string_field",
                    SomeIpStringTypeDefinition {
                        length_field_size: LenFieldSize::OneByte,
                        bom: StringType::UTF_8,
                        fixed_length: None,
                    },
                ),
                get_array_field_definition(
                    "array_field",
                    SomeIpArrayTypeDefinition {
                        fixed_length: None,
                        length_field_size: LenFieldSize::OneByte,
                        array_type: Box::new(SomeIpMessageTypeDefinition::PRIMITIVE(
                            SomeIpPrimitiveTypeDefinition::U32,
                        )),
                    },
                ),
                SomeIpStructFieldTypeDefinition {
                    data_tag: None,
                    field_name: "struct_field".to_string(),
                    field_type: SomeIpMessageTypeDefinition::STRUCT(SomeIpStructTypeDefinition {
                        name: "".to_string(),
                        length_field_size: LenFieldSize::ZeroBytes,
                        fields: [
                            get_primitive_field_definition("a", SomeIpPrimitiveTypeDefinition::U64),
                            get_primitive_field_definition("b", SomeIpPrimitiveTypeDefinition::F64),
                            get_primitive_field_definition("c", SomeIpPrimitiveTypeDefinition::U32),
                        ]
                        .to_vec(),
                    }),
                },
            ]
            .to_vec(),
        };
        const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];

        let text = String::from("hello");
        let text_bytes = text.as_bytes();
        let mut data = Vec::new();
        // 3 bytes for BOM length + 5 bytes for "hello" string bytes + 1 for null byte
        data.push(3 + 5 + 1);
        data.extend_from_slice(BOM_UTF8);
        data.extend_from_slice(text_bytes);
        data.push(0x00);

        let array_data: Vec<u32> = vec![12, 43, 78];
        data.push(12); // array size is 12 bytes
        for element in &array_data {
            data.extend_from_slice(&element.to_be_bytes());
        }

        let u64_val = 54u64;
        let f64_val = 8.23f64;
        let u32_val = 12u32;

        data.extend_from_slice(&u64_val.to_be_bytes());
        data.extend_from_slice(&f64_val.to_be_bytes());
        data.extend_from_slice(&u32_val.to_be_bytes());

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&data);
        let message_descriptor = TestStruct::descriptor();

        let struct_value_seed =
            StructValueSeed::new(&struct_definition, &message_descriptor, Endianness::BigEndian);
        let deserialized_msg =
            struct_value_seed.deserialize(&mut deserializer).expect("Deserialization must succeed");

        let string_field_descriptor = message_descriptor
            .field_by_name("string_field")
            .expect("Field 'string_field' must be available");
        let array_field_descriptor = message_descriptor
            .field_by_name("array_field")
            .expect("Field 'array_field' must be available");
        let struct_field_descriptor = message_descriptor
            .field_by_name("struct_field")
            .expect("Field 'struct_field' must be available");

        assert_eq!(
            string_field_descriptor.get_singular_field_or_default(&*deserialized_msg),
            ReflectValueRef::String(&text)
        );

        let repeated_values = array_field_descriptor.get_repeated(&*deserialized_msg);
        assert_eq!(3, repeated_values.len());
        assert_eq!(RuntimeType::U32, repeated_values.element_type());

        assert_eq!(repeated_values.get(0), ReflectValueRef::U32(array_data[0]));
        assert_eq!(repeated_values.get(1), ReflectValueRef::U32(array_data[1]));
        assert_eq!(repeated_values.get(2), ReflectValueRef::U32(array_data[2]));

        let struct_value = struct_field_descriptor
            .get_singular_field_or_default(&*deserialized_msg)
            .to_message()
            .expect("Field 'struct_field' must be available");

        let message_descriptor = SimpleTestMessage::descriptor();

        let u64_field_descriptor =
            message_descriptor.field_by_name("a").expect("Field 'a' must be available");
        let f64_field_descriptor =
            message_descriptor.field_by_name("b").expect("Field 'b' must be available");
        let u32_field_descriptor =
            message_descriptor.field_by_name("c").expect("Field 'c' must be available");

        assert_eq!(
            u64_field_descriptor.get_singular_field_or_default(&*struct_value),
            ReflectValueRef::U64(u64_val)
        );
        assert_eq!(
            f64_field_descriptor.get_singular_field_or_default(&*struct_value),
            ReflectValueRef::F64(f64_val)
        );
        assert_eq!(
            u32_field_descriptor.get_singular_field_or_default(&*struct_value),
            ReflectValueRef::U32(u32_val)
        );
    }

    #[test]
    fn test_deserialize_simple_struct_with_unknown_field_fails() {
        let struct_definition = SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::ZeroBytes,
            fields: [
                get_primitive_field_definition("a", SomeIpPrimitiveTypeDefinition::U64),
                get_primitive_field_definition("unknown", SomeIpPrimitiveTypeDefinition::U32),
            ]
            .to_vec(),
        };

        let u64_val = 54u64;
        let u32_val = 12u32;

        let mut struct_payload = Vec::new();
        struct_payload.extend_from_slice(&u64_val.to_le_bytes());
        struct_payload.extend_from_slice(&u32_val.to_le_bytes());

        let mut deserializer = SomeIpSerdeDeserializer::<LittleEndian>::new(&struct_payload);
        let message_descriptor = SimpleTestMessage::descriptor();

        let struct_value_seed =
            StructValueSeed::new(&struct_definition, &message_descriptor, Endianness::LittleEndian);
        assert!(struct_value_seed.deserialize(&mut deserializer).is_err());
    }

    #[test]
    fn test_deserialize_simple_struct_length_with_unknown_field_fails() {
        let struct_definition = SomeIpStructTypeDefinition {
            name: "".to_string(),
            length_field_size: LenFieldSize::OneByte,
            fields: [get_primitive_field_definition("unknown", SomeIpPrimitiveTypeDefinition::U32)]
                .to_vec(),
        };

        let u64_val = 54u64;

        let mut struct_payload = Vec::new();
        struct_payload.push(8);
        struct_payload.extend_from_slice(&u64_val.to_be_bytes());

        let mut deserializer = SomeIpSerdeDeserializer::<BigEndian>::new(&struct_payload);
        let message_descriptor = SimpleTestMessage::descriptor();

        let struct_value_seed =
            StructValueSeed::new(&struct_definition, &message_descriptor, Endianness::BigEndian);
        assert!(struct_value_seed.deserialize(&mut deserializer).is_err());
    }
}
