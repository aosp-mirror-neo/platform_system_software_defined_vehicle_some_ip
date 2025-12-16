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

use sdv_wire_format::reflection::codec::api::protobuf::IProtobufWriter;

use crate::translator::direct::{incoming::reader::ISomeIpReader, SomeIpSizeType};

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct StringTypeMetadata {
    pub(crate) someip_size_type: SomeIpSizeType,
}

impl StringTypeMetadata {
    pub(crate) fn new(someip_size_type: SomeIpSizeType) -> Self {
        Self { someip_size_type }
    }

    pub(crate) fn translate_data<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        &self,
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let someip_string_byte_size =
            self.someip_size_type.get_size_from_someip_format(someip_reader)?;
        let value = someip_reader.read_string(someip_string_byte_size)?;
        protobuf_writer
            .write_string(proto_field_number, &value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing string - {e:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::direct::incoming::SomeIpReader;
    use crate::translator::direct::SomeIpSizeType;
    use byteorder::{BigEndian, WriteBytesExt};
    use sdv_wire_format::reflection::codec::api::protobuf::PublicationWriter;
    use someip_config::serde_config::ffi::LenFieldSize;

    #[test]
    fn test_new_fixed() {
        let size_type = SomeIpSizeType::FIXED(10);
        let metadata = StringTypeMetadata::new(size_type);
        assert_eq!(metadata.someip_size_type, size_type);
    }

    #[test]
    fn test_new_variable() {
        let size_type = SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes);
        let metadata = StringTypeMetadata::new(size_type);
        assert_eq!(metadata.someip_size_type, size_type);
    }

    fn test_translate_string(
        metadata: StringTypeMetadata,
        input_bytes: &[u8],
        field_number: u32,
        expected_string: &str,
    ) {
        // Setup
        let mut someip_reader = SomeIpReader::<BigEndian>::new(input_bytes);

        let mut actual_buffer = vec![0; 51];
        let mut protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut actual_buffer).unwrap();

        // Execute
        metadata.translate_data(&mut someip_reader, field_number, &mut protobuf_writer).unwrap();

        // Verify
        let mut expected_buffer = vec![0; 51];
        let mut expected_protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut expected_buffer).unwrap();
        expected_protobuf_writer.write_string(field_number, expected_string).unwrap();

        drop(protobuf_writer);
        drop(expected_protobuf_writer);

        assert_eq!(actual_buffer, expected_buffer);
    }

    #[test]
    fn test_translate_data_fixed_size_utf8() {
        let string_value = "hello";
        let mut someip_payload = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        someip_payload.extend_from_slice(string_value.as_bytes());
        someip_payload.push(0); // Null terminator
        let total_size = someip_payload.len();

        let metadata = StringTypeMetadata::new(SomeIpSizeType::FIXED(total_size));
        let field_number = 1;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }

    #[test]
    fn test_translate_data_variable_size_one_byte_len() {
        let string_value = "world";
        let mut string_data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        string_data.extend_from_slice(string_value.as_bytes());
        string_data.push(0); // Null terminator
        let string_size = string_data.len();

        let mut someip_payload = vec![];
        someip_payload.write_u8(string_size as u8).unwrap();
        someip_payload.extend_from_slice(&string_data);

        let metadata = StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::OneByte));
        let field_number = 2;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }

    #[test]
    fn test_translate_data_variable_size_two_byte_len() {
        let string_value = "another";
        let mut string_data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        string_data.extend_from_slice(string_value.as_bytes());
        string_data.push(0); // Null terminator
        let string_size = string_data.len();

        let mut someip_payload = vec![];
        someip_payload.write_u16::<BigEndian>(string_size as u16).unwrap();
        someip_payload.extend_from_slice(&string_data);

        let metadata = StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes));
        let field_number = 3;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }

    #[test]
    fn test_translate_data_variable_size_four_byte_len() {
        let string_value = "test";
        let mut string_data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        string_data.extend_from_slice(string_value.as_bytes());
        string_data.push(0); // Null terminator
        let string_size = string_data.len();

        let mut someip_payload = vec![];
        someip_payload.write_u32::<BigEndian>(string_size as u32).unwrap();
        someip_payload.extend_from_slice(&string_data);

        let metadata = StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::FourBytes));
        let field_number = 4;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }

    #[test]
    fn test_translate_data_fixed_size_utf16_be() {
        let string_value = "test_utf16";
        let mut someip_payload = vec![0xFE, 0xFF]; // UTF-16 BE BOM
        for c in string_value.encode_utf16() {
            someip_payload.extend_from_slice(&c.to_be_bytes());
        }
        someip_payload.extend_from_slice(&[0, 0]); // Null terminator
        let total_size = someip_payload.len();

        let metadata = StringTypeMetadata::new(SomeIpSizeType::FIXED(total_size));
        let field_number = 5;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }

    #[test]
    fn test_translate_data_fixed_size_utf16_le() {
        let string_value = "test_utf16_le";
        let mut someip_payload = vec![0xFF, 0xFE]; // UTF-16 LE BOM
        for c in string_value.encode_utf16() {
            someip_payload.extend_from_slice(&c.to_le_bytes());
        }
        someip_payload.extend_from_slice(&[0, 0]); // Null terminator
        let total_size = someip_payload.len();

        let metadata = StringTypeMetadata::new(SomeIpSizeType::FIXED(total_size));
        let field_number = 6;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }

    #[test]
    fn test_translate_data_variable_size_utf16_be() {
        let string_value = "var_utf16_be";
        let mut string_data = vec![0xFE, 0xFF]; // UTF-16 BE BOM
        for c in string_value.encode_utf16() {
            string_data.extend_from_slice(&c.to_be_bytes());
        }
        string_data.extend_from_slice(&[0, 0]); // Null terminator
        let string_size = string_data.len();

        let mut someip_payload = vec![];
        someip_payload.write_u16::<BigEndian>(string_size as u16).unwrap();
        someip_payload.extend_from_slice(&string_data);

        let metadata = StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes));
        let field_number = 7;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }

    #[test]
    fn test_translate_data_variable_size_utf16_le() {
        let string_value = "var_utf16_le";
        let mut string_data = vec![0xFF, 0xFE]; // UTF-16 LE BOM
        for c in string_value.encode_utf16() {
            string_data.extend_from_slice(&c.to_le_bytes());
        }
        string_data.extend_from_slice(&[0, 0]); // Null terminator
        let string_size = string_data.len();

        let mut someip_payload = vec![];
        someip_payload.write_u16::<BigEndian>(string_size as u16).unwrap();
        someip_payload.extend_from_slice(&string_data);

        let metadata = StringTypeMetadata::new(SomeIpSizeType::VARIABLE(LenFieldSize::TwoBytes));
        let field_number = 8;

        test_translate_string(metadata, &someip_payload, field_number, string_value);
    }
}
