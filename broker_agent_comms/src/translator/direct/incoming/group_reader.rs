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

use anyhow::bail;
use byteorder::ByteOrder;
use std::borrow::Cow;

use crate::translator::direct::{incoming::SomeIpReader, ISomeIpReader};

// This reader is constructed from multiple SOME/IP readers of contiguous data. Each internal reader
// represents a specific field of the message that this reader is supposed to read. A key characteristic
// is that fields DO NOT cross over readers, i.e. this reader does not support having a part of a field's
// data in one reader, and part in the next. In particular, this influences the implementation of
// `available_for_unchecked_reads`, which only returns the available bytes to read for a single field
#[derive(Debug)]
pub(crate) struct SomeIpGroupReader<'input_lt, 'metadata_lt, B: ByteOrder> {
    someip_data: &'input_lt [u8],
    all_field_indices: &'metadata_lt [usize],
    group_part_fields: &'metadata_lt [usize],
    current_field_index: usize,
    current_reader: SomeIpReader<'input_lt, B>,
}

impl<'input_lt, 'metadata_lt, B: ByteOrder> SomeIpGroupReader<'input_lt, 'metadata_lt, B> {
    pub(crate) fn new(
        someip_data: &'input_lt [u8],
        all_field_indices: &'metadata_lt [usize],
        group_part_fields: &'metadata_lt [usize],
    ) -> anyhow::Result<Self> {
        if group_part_fields.is_empty() {
            // create an empty reader. Any attempt to read will return an error
            Ok(SomeIpGroupReader {
                someip_data,
                all_field_indices,
                group_part_fields,
                current_field_index: 0,
                current_reader: SomeIpReader::new(&[]),
            })
        } else {
            let first_field = group_part_fields[0];
            let (field_start, field_end) =
                Self::get_safe_field_range(someip_data, all_field_indices, first_field)?;

            let current_reader = SomeIpReader::new(&someip_data[field_start..field_end]);

            Ok(SomeIpGroupReader {
                someip_data,
                all_field_indices,
                group_part_fields,
                current_field_index: 0,
                current_reader,
            })
        }
    }

    fn get_safe_field_range(
        someip_data: &[u8],
        all_field_indices: &[usize],
        field_index: usize,
    ) -> anyhow::Result<(usize, usize)> {
        let field_start = *all_field_indices.get(field_index).ok_or_else(|| {
            anyhow::anyhow!(
                "Group event metadata refers to non-existent field number! Wanted field number {} but only {} fields exist",
                field_index,
                all_field_indices.len()
            )
        })?;
        let field_end = *all_field_indices.get(field_index + 1).unwrap_or(&someip_data.len());

        if field_start > someip_data.len() {
            anyhow::bail!(
                "Field start index {} exceeds someip_data length {}",
                field_start,
                someip_data.len()
            );
        }
        if field_end > someip_data.len() {
            anyhow::bail!(
                "Field end index {} exceeds someip_data length {}",
                field_end,
                someip_data.len()
            );
        }
        if field_start > field_end {
            anyhow::bail!(
                "Field start index {} is greater than field end index {}",
                field_start,
                field_end
            );
        }
        Ok((field_start, field_end))
    }

    fn check_reader_index(&self) -> anyhow::Result<()> {
        if self.current_field_index >= self.group_part_fields.len() {
            bail!("Reader index out of bounds")
        }
        Ok(())
    }

    fn advance_to_next_sub_reader(&mut self) -> anyhow::Result<()> {
        self.current_field_index += 1;
        self.check_reader_index()?;
        let current_field = self.group_part_fields[self.current_field_index];
        let (field_start, field_end) =
            Self::get_safe_field_range(self.someip_data, self.all_field_indices, current_field)?;

        self.current_reader = SomeIpReader::new(&self.someip_data[field_start..field_end]);
        Ok(())
    }

    // Helper to advance to the next reader that has data.
    fn advance_to_next_non_empty_reader(&mut self) -> anyhow::Result<()> {
        while self.current_reader.remaining_bytes() == 0 {
            self.advance_to_next_sub_reader()?;
        }
        Ok(())
    }
}

impl<'input_lt, 'metadata_lt, B: ByteOrder> ISomeIpReader<'input_lt>
    for SomeIpGroupReader<'input_lt, 'metadata_lt, B>
{
    fn available_for_unchecked_reads(&mut self, num_bytes: usize) -> anyhow::Result<()> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.available_for_unchecked_reads(num_bytes)
    }

    fn read_bool(&mut self) -> anyhow::Result<bool> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_bool()
    }

    fn read_i8(&mut self) -> anyhow::Result<i8> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_i8()
    }

    fn read_u8(&mut self) -> anyhow::Result<u8> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_u8()
    }

    fn read_i16(&mut self) -> anyhow::Result<i16> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_i16()
    }

    fn read_i32(&mut self) -> anyhow::Result<i32> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_i32()
    }

    fn read_i64(&mut self) -> anyhow::Result<i64> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_i64()
    }

    fn read_u16(&mut self) -> anyhow::Result<u16> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_u16()
    }

    fn read_u32(&mut self) -> anyhow::Result<u32> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_u32()
    }

    fn read_u64(&mut self) -> anyhow::Result<u64> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_u64()
    }

    fn read_f32(&mut self) -> anyhow::Result<f32> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_f32()
    }

    fn read_f64(&mut self) -> anyhow::Result<f64> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_f64()
    }

    fn read_bool_unchecked(&mut self) -> bool {
        self.current_reader.read_bool_unchecked()
    }

    fn read_i8_unchecked(&mut self) -> i8 {
        self.current_reader.read_i8_unchecked()
    }

    fn read_u8_unchecked(&mut self) -> u8 {
        self.current_reader.read_u8_unchecked()
    }

    fn read_i16_unchecked(&mut self) -> i16 {
        self.current_reader.read_i16_unchecked()
    }

    fn read_i32_unchecked(&mut self) -> i32 {
        self.current_reader.read_i32_unchecked()
    }

    fn read_i64_unchecked(&mut self) -> i64 {
        self.current_reader.read_i64_unchecked()
    }

    fn read_u16_unchecked(&mut self) -> u16 {
        self.current_reader.read_u16_unchecked()
    }

    fn read_u32_unchecked(&mut self) -> u32 {
        self.current_reader.read_u32_unchecked()
    }

    fn read_u64_unchecked(&mut self) -> u64 {
        self.current_reader.read_u64_unchecked()
    }

    fn read_f32_unchecked(&mut self) -> f32 {
        self.current_reader.read_f32_unchecked()
    }

    fn read_f64_unchecked(&mut self) -> f64 {
        self.current_reader.read_f64_unchecked()
    }

    fn read_string(&mut self, string_size: usize) -> anyhow::Result<Cow<'input_lt, str>> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_string(string_size)
    }

    fn read_bytes_array(&mut self, size: usize) -> anyhow::Result<&'input_lt [u8]> {
        self.advance_to_next_non_empty_reader()?;
        self.current_reader.read_bytes_array(size)
    }

    fn num_read_bytes(&self) -> usize {
        // Sum of bytes read from all readers up to and including the current one.
        let mut sum = 0;
        for i in 0..self.current_field_index {
            let field = self.group_part_fields[i];
            // Use .get() to safely access field indices - if they don't exist, skip this field
            let Some(&field_start) = self.all_field_indices.get(field) else {
                continue;
            };
            let field_end =
                *self.all_field_indices.get(field + 1).unwrap_or(&self.someip_data.len());
            // Ensure we don't go beyond someip_data bounds
            let field_end = field_end.min(self.someip_data.len());
            let field_start = field_start.min(self.someip_data.len());
            sum += field_end.saturating_sub(field_start);
        }
        sum += self.current_reader.num_read_bytes();
        sum
    }

    fn advance_by(&mut self, num_bytes: usize) -> usize {
        let mut remaining_to_skip = num_bytes;

        while remaining_to_skip > 0 {
            let skipped = self.current_reader.advance_by(remaining_to_skip);
            remaining_to_skip -= skipped;

            if self.current_reader.remaining_bytes() == 0 {
                if self.advance_to_next_sub_reader().is_err() {
                    // We reached the end of valid input for the group part
                    break;
                }
            } else {
                // Current reader still has more data, stop skipping bytes and return
                break;
            }
        }
        num_bytes - remaining_to_skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::BigEndian;

    #[test]
    fn test_read_from_single_reader() {
        let data = vec![1, 2, 3, 4];
        let mut group_reader = SomeIpGroupReader::<BigEndian>::new(&data, &[0], &[0]).unwrap();

        assert_eq!(group_reader.read_u8().unwrap(), 1);
        assert_eq!(group_reader.read_u8().unwrap(), 2);
        assert_eq!(group_reader.read_u16().unwrap(), 0x0304);
        assert!(group_reader.read_u8().is_err());
    }

    #[test]
    fn test_read_across_multiple_readers() {
        let field1 = vec![1, 2];
        let field2 = vec![3, 4, 5, 6];
        let mut data = Vec::new();
        data.extend_from_slice(&field1);
        data.extend_from_slice(&field2);
        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2], &[0, 1]).unwrap();

        assert_eq!(group_reader.read_u16().unwrap(), 0x0102);
        assert_eq!(group_reader.read_u32().unwrap(), 0x03040506);
        assert!(group_reader.read_u8().is_err());
    }

    #[test]
    fn test_read_with_empty_readers() {
        let field1 = vec![1];
        let field2 = vec![];
        let field3 = vec![2];
        let mut data = Vec::new();
        data.extend_from_slice(&field1);
        data.extend_from_slice(&field2);
        data.extend_from_slice(&field3);
        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 1, 1], &[0, 1, 2]).unwrap();

        assert_eq!(group_reader.read_u8().unwrap(), 1);
        assert_eq!(group_reader.read_u8().unwrap(), 2);
        assert!(group_reader.read_u8().is_err());
    }

    #[test]
    fn test_num_read_bytes() {
        let field1 = vec![1, 2];
        let field2 = vec![3, 4];
        let mut data = Vec::new();
        data.extend_from_slice(&field1);
        data.extend_from_slice(&field2);
        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2], &[0, 1]).unwrap();
        assert_eq!(group_reader.num_read_bytes(), 0);
        group_reader.read_u8().unwrap();
        assert_eq!(group_reader.num_read_bytes(), 1);
        group_reader.read_u8().unwrap();
        assert_eq!(group_reader.num_read_bytes(), 2);
        group_reader.read_u8().unwrap();
        assert_eq!(group_reader.num_read_bytes(), 3);
    }

    #[test]
    fn test_advance_by() {
        let field1 = vec![1, 2, 3];
        let field2 = vec![4, 5, 6];
        let mut data = Vec::new();
        data.extend_from_slice(&field1);
        data.extend_from_slice(&field2);

        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 3], &[0, 1]).unwrap();

        assert_eq!(group_reader.advance_by(2), 2);
        assert_eq!(group_reader.read_u8().unwrap(), 3);

        assert_eq!(group_reader.advance_by(2), 2);
        assert_eq!(group_reader.read_u8().unwrap(), 6);

        assert_eq!(group_reader.advance_by(10), 0);
    }

    #[test]
    fn test_advance_by_across_readers() {
        let field1 = vec![1, 2];
        let field2 = vec![3, 4, 5];
        let mut data = Vec::new();
        data.extend_from_slice(&field1);
        data.extend_from_slice(&field2);
        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2], &[0, 1]).unwrap();

        assert_eq!(group_reader.advance_by(4), 4);
        assert_eq!(group_reader.read_u8().unwrap(), 5);
    }

    #[test]
    fn test_available_for_unchecked_reads() {
        let field1 = vec![1, 2];
        let field2 = vec![3, 4, 5, 6];
        let mut data = Vec::new();
        data.extend_from_slice(&field1);
        data.extend_from_slice(&field2);
        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2], &[0, 1]).unwrap();

        assert!(group_reader.available_for_unchecked_reads(2).is_ok());
        assert!(group_reader.available_for_unchecked_reads(3).is_err());

        group_reader.read_u16().unwrap();

        assert!(group_reader.available_for_unchecked_reads(4).is_ok());
        assert!(group_reader.available_for_unchecked_reads(5).is_err());
    }

    #[test]
    fn test_read_unchecked() {
        let field1 = vec![1, 2];
        let field2 = vec![3, 4, 5, 6];
        let mut data = Vec::new();
        data.extend_from_slice(&field1);
        data.extend_from_slice(&field2);
        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2], &[0, 1]).unwrap();

        group_reader.available_for_unchecked_reads(2).unwrap();
        assert_eq!(group_reader.read_u16_unchecked(), 0x0102);

        group_reader.available_for_unchecked_reads(4).unwrap();
        assert_eq!(group_reader.read_u32_unchecked(), 0x03040506);
    }

    #[test]
    fn test_read_shorter_data_than_metadata() {
        let data = vec![1, 2, 3]; // Only 3 bytes, but metadata expects 2 fields of 2 bytes each (total 4)
        let result = SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2], &[0, 1]);

        // Construction might still succeed if only the first field is accessed and it fits
        if let Ok(mut group_reader) = result {
            assert_eq!(group_reader.read_u16().unwrap(), 0x0102);
            // Advancing to the next field should fail because it starts at index 2 and expects 2 bytes, but only 1 remains.
            assert!(group_reader.read_u16().is_err());
        }
    }

    #[test]
    fn test_construction_fails_if_first_field_out_of_bounds() {
        let data = vec![1]; // Only 1 byte

        // Metadata says first field starts at 0 and ends at 2.
        let result = SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2], &[0, 1]);
        assert!(result.is_err());
    }

    #[test]
    fn test_fails_if_metadata_not_monotonic() {
        let data = vec![1, 2, 3, 4];
        // Metadata says first field starts at 2 and ends at 1.
        let result = SomeIpGroupReader::<BigEndian>::new(&data, &[2, 1], &[0]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is greater than field end index"));
    }

    #[test]
    fn test_advance_to_invalid_metadata_fails() {
        let data = vec![1, 2, 3, 4];
        // First field is valid [0..2], second field is invalid [5..6]
        let mut group_reader =
            SomeIpGroupReader::<BigEndian>::new(&data, &[0, 2, 5, 6], &[0, 1]).unwrap();

        assert_eq!(group_reader.read_u16().unwrap(), 0x0102);
        // Attempting to read more will try to advance to the next field [5..6], which is out of bounds.
        assert!(group_reader.read_u8().is_err());
    }

    #[test]
    fn test_num_read_bytes_with_broken_metadata() {
        let data = vec![1, 2];
        // Test that 'new' fails if the first field has invalid indices.
        let result = SomeIpGroupReader::<BigEndian>::new(&data, &[10, 5, 0, 1], &[0, 1]);
        assert!(result.is_err());
    }
}
