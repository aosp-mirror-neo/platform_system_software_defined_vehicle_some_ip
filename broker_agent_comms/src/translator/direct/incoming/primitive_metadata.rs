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
use crate::translator::direct::{
    incoming::reader::ISomeIpReader,
    valid_mappings::{TRANSLATION_OPERATION_INDICES, VALID_PRIMITIVE_TYPE_MAPPINGS},
};
use anyhow::bail;
use protobuf::descriptor::field_descriptor_proto::Type;
use sdv_wire_format::reflection::codec::api::protobuf::IProtobufWriter;
use someip_serde_reflect::SomeIpPrimitiveTypeDefinition;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PrimitiveTypeMetadata {
    translate_operation_index: u8,
}

impl PrimitiveTypeMetadata {
    pub(crate) fn new(
        someip_primitive_type: SomeIpPrimitiveTypeDefinition,
        protobuf_field_type: Type,
    ) -> anyhow::Result<Self> {
        let translate_operation_index =
            Self::get_translate_operation_index(someip_primitive_type, protobuf_field_type)?;
        Ok(Self { translate_operation_index })
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
        match self.translate_operation_index {
            0 => Self::translate_bool(someip_reader, proto_field_number, protobuf_writer),
            1 => Self::translate_u8_to_uint32(someip_reader, proto_field_number, protobuf_writer),
            2 => Self::translate_u8_to_enum(someip_reader, proto_field_number, protobuf_writer),
            3 => Self::translate_u8_to_fixed32(someip_reader, proto_field_number, protobuf_writer),
            4 => Self::translate_u8_to_uint64(someip_reader, proto_field_number, protobuf_writer),
            5 => Self::translate_u8_to_fixed64(someip_reader, proto_field_number, protobuf_writer),
            6 => Self::translate_u16_to_uint32(someip_reader, proto_field_number, protobuf_writer),
            7 => Self::translate_u16_to_enum(someip_reader, proto_field_number, protobuf_writer),
            8 => Self::translate_u16_to_fixed32(someip_reader, proto_field_number, protobuf_writer),
            9 => Self::translate_u16_to_uint64(someip_reader, proto_field_number, protobuf_writer),
            10 => {
                Self::translate_u16_to_fixed64(someip_reader, proto_field_number, protobuf_writer)
            }
            11 => Self::translate_u32_to_uint32(someip_reader, proto_field_number, protobuf_writer),
            12 => {
                Self::translate_u32_to_fixed32(someip_reader, proto_field_number, protobuf_writer)
            }
            13 => Self::translate_u32_to_uint64(someip_reader, proto_field_number, protobuf_writer),
            14 => {
                Self::translate_u32_to_fixed64(someip_reader, proto_field_number, protobuf_writer)
            }
            15 => Self::translate_u64_to_uint64(someip_reader, proto_field_number, protobuf_writer),
            16 => {
                Self::translate_u64_to_fixed64(someip_reader, proto_field_number, protobuf_writer)
            }
            17 => Self::translate_i8_to_sint32(someip_reader, proto_field_number, protobuf_writer),
            18 => Self::translate_i8_to_int32(someip_reader, proto_field_number, protobuf_writer),
            19 => Self::translate_i8_to_enum(someip_reader, proto_field_number, protobuf_writer),
            20 => {
                Self::translate_i8_to_sfixed32(someip_reader, proto_field_number, protobuf_writer)
            }
            21 => Self::translate_i8_to_sint64(someip_reader, proto_field_number, protobuf_writer),
            22 => Self::translate_i8_to_int64(someip_reader, proto_field_number, protobuf_writer),
            23 => {
                Self::translate_i8_to_sfixed64(someip_reader, proto_field_number, protobuf_writer)
            }
            24 => Self::translate_i16_to_int32(someip_reader, proto_field_number, protobuf_writer),
            25 => Self::translate_i16_to_sint32(someip_reader, proto_field_number, protobuf_writer),
            26 => Self::translate_i16_to_enum(someip_reader, proto_field_number, protobuf_writer),
            27 => {
                Self::translate_i16_to_sfixed32(someip_reader, proto_field_number, protobuf_writer)
            }
            28 => Self::translate_i16_to_int64(someip_reader, proto_field_number, protobuf_writer),
            29 => Self::translate_i16_to_sint64(someip_reader, proto_field_number, protobuf_writer),
            30 => {
                Self::translate_i16_to_sfixed64(someip_reader, proto_field_number, protobuf_writer)
            }
            31 => Self::translate_i32_to_int32(someip_reader, proto_field_number, protobuf_writer),
            32 => Self::translate_i32_to_sint32(someip_reader, proto_field_number, protobuf_writer),
            33 => Self::translate_i32_to_enum(someip_reader, proto_field_number, protobuf_writer),
            34 => {
                Self::translate_i32_to_sfixed32(someip_reader, proto_field_number, protobuf_writer)
            }
            35 => Self::translate_i32_to_int64(someip_reader, proto_field_number, protobuf_writer),
            36 => Self::translate_i32_to_sint64(someip_reader, proto_field_number, protobuf_writer),
            37 => {
                Self::translate_i32_to_sfixed64(someip_reader, proto_field_number, protobuf_writer)
            }
            38 => Self::translate_i64_to_int64(someip_reader, proto_field_number, protobuf_writer),
            39 => Self::translate_i64_to_sint64(someip_reader, proto_field_number, protobuf_writer),
            40 => {
                Self::translate_i64_to_sfixed64(someip_reader, proto_field_number, protobuf_writer)
            }
            41 => Self::translate_f32_to_float(someip_reader, proto_field_number, protobuf_writer),
            42 => Self::translate_f32_to_double(someip_reader, proto_field_number, protobuf_writer),
            43 => Self::translate_f64_to_double(someip_reader, proto_field_number, protobuf_writer),
            _ => bail!("Unknown translation operation index: {}", self.translate_operation_index),
        }
    }

    fn translate_bool<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_bool()?;
        protobuf_writer
            .write_bool(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing bool - {e:?}")))
    }

    fn translate_u8_to_uint32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u8()?;
        protobuf_writer
            .write_uint32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing uint32 - {e:?}")))
    }

    fn translate_u8_to_enum<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u8()?;
        protobuf_writer
            .write_enum(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing enum - {e:?}")))
    }

    fn translate_u8_to_fixed32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u8()?;
        protobuf_writer
            .write_fixed32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing fixed32 - {e:?}")))
    }

    fn translate_u8_to_uint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u8()?;
        protobuf_writer
            .write_uint64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing uint64 - {e:?}")))
    }

    fn translate_u8_to_fixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u8()?;
        protobuf_writer
            .write_fixed64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing fixed64 - {e:?}")))
    }

    fn translate_u16_to_uint32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u16()?;
        protobuf_writer
            .write_uint32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing uint32 - {e:?}")))
    }

    fn translate_u16_to_enum<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u16()?;
        protobuf_writer
            .write_enum(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing enum - {e:?}")))
    }

    fn translate_u16_to_fixed32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u16()?;
        protobuf_writer
            .write_fixed32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing fixed32 - {e:?}")))
    }

    fn translate_u16_to_uint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u16()?;
        protobuf_writer
            .write_uint64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing uint64 - {e:?}")))
    }

    fn translate_u16_to_fixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u16()?;
        protobuf_writer
            .write_fixed64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing fixed64 - {e:?}")))
    }

    fn translate_u32_to_uint32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u32()?;
        protobuf_writer
            .write_uint32(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing uint32 - {e:?}")))
    }

    fn translate_u32_to_fixed32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u32()?;
        protobuf_writer
            .write_fixed32(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing fixed32 - {e:?}")))
    }

    fn translate_u32_to_uint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u32()?;
        protobuf_writer
            .write_uint64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing uint64 - {e:?}")))
    }

    fn translate_u32_to_fixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u32()?;
        protobuf_writer
            .write_fixed64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing fixed64 - {e:?}")))
    }

    fn translate_u64_to_uint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u64()?;
        protobuf_writer
            .write_uint64(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing uint64 - {e:?}")))
    }

    fn translate_u64_to_fixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_u64()?;
        protobuf_writer
            .write_fixed64(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing fixed64 - {e:?}")))
    }

    fn translate_i8_to_sint32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i8()?;
        protobuf_writer
            .write_sint32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sint32 - {e:?}")))
    }

    fn translate_i8_to_int32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i8()?;
        protobuf_writer
            .write_int32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing int32 - {e:?}")))
    }

    fn translate_i8_to_enum<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i8()?;
        protobuf_writer
            .write_enum(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing enum - {e:?}")))
    }

    fn translate_i8_to_sfixed32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i8()?;
        protobuf_writer
            .write_sfixed32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sfixed32 - {e:?}")))
    }

    fn translate_i8_to_sint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i8()?;
        protobuf_writer
            .write_sint64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sint64 - {e:?}")))
    }

    fn translate_i8_to_int64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i8()?;
        protobuf_writer
            .write_int64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing int64 - {e:?}")))
    }

    fn translate_i8_to_sfixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i8()?;
        protobuf_writer
            .write_sfixed64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sfixed64 - {e:?}")))
    }

    fn translate_i16_to_int32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i16()?;
        protobuf_writer
            .write_int32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing int32 - {e:?}")))
    }

    fn translate_i16_to_sint32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i16()?;
        protobuf_writer
            .write_sint32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sint32 - {e:?}")))
    }

    fn translate_i16_to_enum<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i16()?;
        protobuf_writer
            .write_enum(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing enum - {e:?}")))
    }

    fn translate_i16_to_sfixed32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i16()?;
        protobuf_writer
            .write_sfixed32(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sfixed32 - {e:?}")))
    }

    fn translate_i16_to_int64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i16()?;
        protobuf_writer
            .write_int64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing int64 - {e:?}")))
    }

    fn translate_i16_to_sint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i16()?;
        protobuf_writer
            .write_sint64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sint64 - {e:?}")))
    }

    fn translate_i16_to_sfixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i16()?;
        protobuf_writer
            .write_sfixed64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sfixed64 - {e:?}")))
    }

    fn translate_i32_to_int32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i32()?;
        protobuf_writer
            .write_int32(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing int32 - {e:?}")))
    }

    fn translate_i32_to_sint32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i32()?;
        protobuf_writer
            .write_sint32(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sint32 - {e:?}")))
    }

    fn translate_i32_to_enum<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i32()?;
        protobuf_writer
            .write_enum(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing enum - {e:?}")))
    }

    fn translate_i32_to_sfixed32<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i32()?;
        protobuf_writer
            .write_sfixed32(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sfixed32 - {e:?}")))
    }

    fn translate_i32_to_int64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i32()?;
        protobuf_writer
            .write_int64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing int64 - {e:?}")))
    }

    fn translate_i32_to_sint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i32()?;
        protobuf_writer
            .write_sint64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sint64 - {e:?}")))
    }

    fn translate_i32_to_sfixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i32()?;
        protobuf_writer
            .write_sfixed64(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sfixed64 - {e:?}")))
    }

    fn translate_i64_to_int64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i64()?;
        protobuf_writer
            .write_int64(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing int64 - {e:?}")))
    }

    fn translate_i64_to_sint64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i64()?;
        protobuf_writer
            .write_sint64(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sint64 - {e:?}")))
    }

    fn translate_i64_to_sfixed64<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_i64()?;
        protobuf_writer
            .write_sfixed64(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing sfixed64 - {e:?}")))
    }

    fn translate_f32_to_float<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_f32()?;
        protobuf_writer
            .write_float(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing float - {e:?}")))
    }

    fn translate_f32_to_double<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_f32()?;
        protobuf_writer
            .write_double(proto_field_number, value.into())
            .map_err(|e| anyhow::Error::msg(format!("Error serializing double - {e:?}")))
    }

    fn translate_f64_to_double<
        'input_lt,
        'output_lt,
        R: ISomeIpReader<'input_lt>,
        W: IProtobufWriter<'output_lt>,
    >(
        someip_reader: &mut R,
        proto_field_number: u32,
        protobuf_writer: &mut W,
    ) -> anyhow::Result<()> {
        let value = someip_reader.read_f64()?;
        protobuf_writer
            .write_double(proto_field_number, value)
            .map_err(|e| anyhow::Error::msg(format!("Error serializing double - {e:?}")))
    }

    fn get_translate_operation_index(
        someip_primitive_type: SomeIpPrimitiveTypeDefinition,
        protobuf_field_type: Type,
    ) -> anyhow::Result<u8> {
        if !VALID_PRIMITIVE_TYPE_MAPPINGS
            .get(&someip_primitive_type)
            .is_some_and(|s| s.contains(&protobuf_field_type))
        {
            bail!(
                "Type mismatch: SOME/IP type {:?} cannot be mapped to proto type {:?}",
                someip_primitive_type,
                protobuf_field_type
            );
        }
        TRANSLATION_OPERATION_INDICES
            .get(&(someip_primitive_type, protobuf_field_type))
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No translation operation found for {:?} -> {:?}",
                    someip_primitive_type,
                    protobuf_field_type
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::direct::incoming::SomeIpReader;
    use byteorder::{BigEndian, WriteBytesExt};
    use sdv_wire_format::reflection::codec::api::protobuf::PublicationWriter;

    fn test_translate_primitive<F>(
        someip_type: SomeIpPrimitiveTypeDefinition,
        proto_type: Type,
        input_bytes: &[u8],
        field_number: u32,
        writer_fn: F,
    ) where
        F: FnOnce(&mut PublicationWriter),
    {
        // Setup
        let metadata = PrimitiveTypeMetadata::new(someip_type, proto_type).unwrap();
        let mut someip_reader = SomeIpReader::<BigEndian>::new(input_bytes);

        let mut actual_buffer = vec![0; 21];
        let mut protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut actual_buffer).unwrap();

        // Execute
        metadata.translate_data(&mut someip_reader, field_number, &mut protobuf_writer).unwrap();
        // Verify
        let mut expected_buffer = vec![0; 21];
        let mut expected_protobuf_writer =
            PublicationWriter::create_publication_writer(1, &mut expected_buffer).unwrap();
        writer_fn(&mut expected_protobuf_writer);

        drop(protobuf_writer);
        drop(expected_protobuf_writer);

        assert_eq!(actual_buffer, expected_buffer);
    }

    #[test]
    fn test_new() {
        let metadata =
            PrimitiveTypeMetadata::new(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32)
                .unwrap();
        let expected_index = TRANSLATION_OPERATION_INDICES
            .get(&(SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32))
            .unwrap();
        assert_eq!(metadata.translate_operation_index, *expected_index);
    }

    #[test]
    fn test_translate_boolean() {
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::BOOLEAN,
            Type::TYPE_BOOL,
            &[1],
            1,
            |writer| writer.write_bool(1, true).unwrap(),
        );
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::BOOLEAN,
            Type::TYPE_BOOL,
            &[0],
            2,
            |writer| writer.write_bool(2, false).unwrap(),
        );
    }

    #[test]
    fn test_translate_u8() {
        let value = 42u8;
        let field_number = 1;
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_UINT32,
            &[value],
            field_number,
            |writer| writer.write_uint32(field_number, value.into()).unwrap(),
        );
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_UINT64,
            &[value],
            field_number,
            |writer| writer.write_uint64(field_number, value.into()).unwrap(),
        );
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_FIXED32,
            &[value],
            field_number,
            |writer| writer.write_fixed32(field_number, value.into()).unwrap(),
        );
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_FIXED64,
            &[value],
            field_number,
            |writer| writer.write_fixed64(field_number, value.into()).unwrap(),
        );
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U8,
            Type::TYPE_ENUM,
            &[value],
            field_number,
            |writer| writer.write_enum(field_number, value.into()).unwrap(),
        );
    }

    #[test]
    fn test_translate_u16() {
        let value = 1000u16;
        let field_number = 2;
        let mut buf = vec![];
        buf.write_u16::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U16,
            Type::TYPE_UINT32,
            &buf,
            field_number,
            |writer| writer.write_uint32(field_number, value.into()).unwrap(),
        );
    }

    #[test]
    fn test_translate_u32() {
        let value = 100_000u32;
        let field_number = 3;
        let mut buf = vec![];
        buf.write_u32::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U32,
            Type::TYPE_UINT32,
            &buf,
            field_number,
            |writer| writer.write_uint32(field_number, value).unwrap(),
        );
    }

    #[test]
    fn test_translate_u64() {
        let value = 1_000_000_000_000u64;
        let field_number = 4;
        let mut buf = vec![];
        buf.write_u64::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::U64,
            Type::TYPE_UINT64,
            &buf,
            field_number,
            |writer| writer.write_uint64(field_number, value).unwrap(),
        );
    }

    #[test]
    fn test_translate_i8() {
        let value = -42i8;
        let field_number = 5;
        let mut buf = vec![];
        buf.write_i8(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::I8,
            Type::TYPE_INT32,
            &buf,
            field_number,
            |writer| writer.write_int32(field_number, value.into()).unwrap(),
        );
    }

    #[test]
    fn test_translate_i16() {
        let value = -1000i16;
        let field_number = 6;
        let mut buf = vec![];
        buf.write_i16::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::I16,
            Type::TYPE_INT32,
            &buf,
            field_number,
            |writer| writer.write_int32(field_number, value.into()).unwrap(),
        );
    }

    #[test]
    fn test_translate_i32() {
        let value = -100_000i32;
        let field_number = 7;
        let mut buf = vec![];
        buf.write_i32::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::I32,
            Type::TYPE_INT32,
            &buf,
            field_number,
            |writer| writer.write_int32(field_number, value).unwrap(),
        );
    }

    #[test]
    fn test_translate_i64() {
        let value = -1_000_000_000_000i64;
        let field_number = 8;
        let mut buf = vec![];
        buf.write_i64::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::I64,
            Type::TYPE_SFIXED64,
            &buf,
            field_number,
            |writer| writer.write_sfixed64(field_number, value).unwrap(),
        );
    }

    #[test]
    fn test_translate_f32() {
        let value = 123.456f32;
        let field_number = 9;
        let mut buf = vec![];
        buf.write_f32::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::F32,
            Type::TYPE_FLOAT,
            &buf,
            field_number,
            |writer| writer.write_float(field_number, value).unwrap(),
        );
    }

    #[test]
    fn test_translate_f64() {
        let value = -123.456789f64;
        let field_number = 10;
        let mut buf = vec![];
        buf.write_f64::<BigEndian>(value).unwrap();
        test_translate_primitive(
            SomeIpPrimitiveTypeDefinition::F64,
            Type::TYPE_DOUBLE,
            &buf,
            field_number,
            |writer| writer.write_double(field_number, value).unwrap(),
        );
    }

    #[test]
    fn test_translation_logic_is_consistent() {
        // This test ensures consistency between the declared valid mappings, the implemented
        // translation operations, and the dispatching logic that connects them. It verifies
        // that the index in `TRANSLATION_OPERATION_INDICES` for a given type mapping
        // correctly corresponds to the appropriate translation function
        // Ground truth map of (SOME/IP type, Protobuf type) -> translation function.

        struct PrimitiveTranslatorTable<'i, 'w, A, B>(
            std::marker::PhantomData<&'i u8>,
            std::marker::PhantomData<&'w u8>,
            std::marker::PhantomData<A>,
            std::marker::PhantomData<B>,
        );

        impl<'i, 'w, A: ISomeIpReader<'i>, B: IProtobufWriter<'w>> PrimitiveTranslatorTable<'i, 'w, A, B> {
            const OPS: [(
                (SomeIpPrimitiveTypeDefinition, Type),
                fn(&mut A, u32, &mut B) -> anyhow::Result<()>,
            ); 44] = [
                (
                    (SomeIpPrimitiveTypeDefinition::BOOLEAN, Type::TYPE_BOOL),
                    PrimitiveTypeMetadata::translate_bool as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT32),
                    PrimitiveTypeMetadata::translate_u8_to_uint32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_ENUM),
                    PrimitiveTypeMetadata::translate_u8_to_enum as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_FIXED32),
                    PrimitiveTypeMetadata::translate_u8_to_fixed32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_UINT64),
                    PrimitiveTypeMetadata::translate_u8_to_uint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U8, Type::TYPE_FIXED64),
                    PrimitiveTypeMetadata::translate_u8_to_fixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_UINT32),
                    PrimitiveTypeMetadata::translate_u16_to_uint32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_ENUM),
                    PrimitiveTypeMetadata::translate_u16_to_enum as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_FIXED32),
                    PrimitiveTypeMetadata::translate_u16_to_fixed32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_UINT64),
                    PrimitiveTypeMetadata::translate_u16_to_uint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U16, Type::TYPE_FIXED64),
                    PrimitiveTypeMetadata::translate_u16_to_fixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT32),
                    PrimitiveTypeMetadata::translate_u32_to_uint32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_FIXED32),
                    PrimitiveTypeMetadata::translate_u32_to_fixed32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_UINT64),
                    PrimitiveTypeMetadata::translate_u32_to_uint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U32, Type::TYPE_FIXED64),
                    PrimitiveTypeMetadata::translate_u32_to_fixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U64, Type::TYPE_UINT64),
                    PrimitiveTypeMetadata::translate_u64_to_uint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::U64, Type::TYPE_FIXED64),
                    PrimitiveTypeMetadata::translate_u64_to_fixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SINT32),
                    PrimitiveTypeMetadata::translate_i8_to_sint32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_INT32),
                    PrimitiveTypeMetadata::translate_i8_to_int32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_ENUM),
                    PrimitiveTypeMetadata::translate_i8_to_enum as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SFIXED32),
                    PrimitiveTypeMetadata::translate_i8_to_sfixed32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SINT64),
                    PrimitiveTypeMetadata::translate_i8_to_sint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_INT64),
                    PrimitiveTypeMetadata::translate_i8_to_int64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I8, Type::TYPE_SFIXED64),
                    PrimitiveTypeMetadata::translate_i8_to_sfixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_INT32),
                    PrimitiveTypeMetadata::translate_i16_to_int32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SINT32),
                    PrimitiveTypeMetadata::translate_i16_to_sint32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_ENUM),
                    PrimitiveTypeMetadata::translate_i16_to_enum as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SFIXED32),
                    PrimitiveTypeMetadata::translate_i16_to_sfixed32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_INT64),
                    PrimitiveTypeMetadata::translate_i16_to_int64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SINT64),
                    PrimitiveTypeMetadata::translate_i16_to_sint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I16, Type::TYPE_SFIXED64),
                    PrimitiveTypeMetadata::translate_i16_to_sfixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_INT32),
                    PrimitiveTypeMetadata::translate_i32_to_int32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SINT32),
                    PrimitiveTypeMetadata::translate_i32_to_sint32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_ENUM),
                    PrimitiveTypeMetadata::translate_i32_to_enum as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SFIXED32),
                    PrimitiveTypeMetadata::translate_i32_to_sfixed32 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_INT64),
                    PrimitiveTypeMetadata::translate_i32_to_int64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SINT64),
                    PrimitiveTypeMetadata::translate_i32_to_sint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I32, Type::TYPE_SFIXED64),
                    PrimitiveTypeMetadata::translate_i32_to_sfixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_INT64),
                    PrimitiveTypeMetadata::translate_i64_to_int64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_SINT64),
                    PrimitiveTypeMetadata::translate_i64_to_sint64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::I64, Type::TYPE_SFIXED64),
                    PrimitiveTypeMetadata::translate_i64_to_sfixed64 as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::F32, Type::TYPE_FLOAT),
                    PrimitiveTypeMetadata::translate_f32_to_float as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::F32, Type::TYPE_DOUBLE),
                    PrimitiveTypeMetadata::translate_f32_to_double as _,
                ),
                (
                    (SomeIpPrimitiveTypeDefinition::F64, Type::TYPE_DOUBLE),
                    PrimitiveTypeMetadata::translate_f64_to_double as _,
                ),
            ];
        }

        let someip_data = 45.43f64.to_be_bytes();
        // 2. Check that every operation in the index map points to the correct function
        //    and corresponds to a valid mapping.
        for (key, index) in TRANSLATION_OPERATION_INDICES.iter() {
            let (someip_type, protobuf_type) =
                PrimitiveTranslatorTable::<SomeIpReader<'_, BigEndian>, PublicationWriter<'_>>::OPS
                    [*index as usize]
                    .0;

            // Check it's a valid mapping
            assert!(
                VALID_PRIMITIVE_TYPE_MAPPINGS
                    .get(&someip_type)
                    .is_some_and(|s| s.contains(&protobuf_type)),
                "TRANSLATION_OPERATION_INDICES contains an invalid mapping: {key:?}"
            );
            // Check the function pointer is correct by comparing the output of their serializations

            let primitive_metadata =
                PrimitiveTypeMetadata::new(someip_type, protobuf_type).unwrap();

            assert_eq!(primitive_metadata.translate_operation_index, *index);

            let mut someip_reader_expected = SomeIpReader::<BigEndian>::new(&someip_data);
            let mut expected_buffer = vec![0; 51];
            let mut protobuf_writer_expected =
                PublicationWriter::create_publication_writer(1, &mut expected_buffer).unwrap();

            let mut someip_reader_actual = SomeIpReader::<BigEndian>::new(&someip_data);
            let mut actual_buffer = vec![0; 51];
            let mut protobuf_writer_actual =
                PublicationWriter::create_publication_writer(1, &mut actual_buffer).unwrap();

            PrimitiveTranslatorTable::<SomeIpReader<'_, BigEndian>, PublicationWriter<'_>>::OPS
                [*index as usize]
                .1(&mut someip_reader_expected, 1, &mut protobuf_writer_expected)
            .expect("Serialization must succeed");

            primitive_metadata
                .translate_data(&mut someip_reader_actual, 1, &mut protobuf_writer_actual)
                .expect("Serialization must succeed");

            drop(protobuf_writer_expected);
            drop(protobuf_writer_actual);

            assert_eq!(actual_buffer, expected_buffer);
        }
        // 3. Check that every valid mapping has an implementation.
        for (someip_type, proto_types) in VALID_PRIMITIVE_TYPE_MAPPINGS.iter() {
            for proto_type in proto_types.iter() {
                let key = (*someip_type, *proto_type);
                assert!(
                    TRANSLATION_OPERATION_INDICES.contains_key(&key),
                    "A valid mapping {key:?} is not present in TRANSLATION_OPERATION_INDICES."
                );
            }
        }
    }
}
