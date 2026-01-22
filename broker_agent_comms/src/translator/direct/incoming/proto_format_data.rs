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

// ProtoFormatData represents a valid protobuf serialized message.
// The provided buffer's suffix is a valid protobuf serialized message. The start index specifies the start
// of this suffix. When `start_index` == `buffer.len()``, this struct represents an empty proto message
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct ProtoFormatData {
    buffer: Vec<u8>,
    start_index: usize,
}

impl ProtoFormatData {
    pub(crate) fn new(buffer: Vec<u8>, start_index: usize) -> anyhow::Result<Self> {
        // `start_index` == buffer.len() IS valid and represents an empty slice, even though
        // buffer[buffer.len()] IS NOT a valid indexing operation.
        if start_index != 0 && start_index > buffer.len() {
            bail!(
                "Invalid start index: {start_index} is greater than buffer length {}",
                buffer.len()
            )
        }
        Ok(ProtoFormatData { buffer, start_index })
    }

    pub(crate) fn get_data(&self) -> &[u8] {
        &self.buffer[self.start_index..]
    }

    pub(crate) fn get_vec(self) -> Vec<u8> {
        if self.buffer.is_empty() || self.start_index == 0 {
            self.buffer
        } else {
            self.buffer[self.start_index..].to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let buffer = vec![1, 2, 3];
        let start_index = 1;
        let data = ProtoFormatData::new(buffer.clone(), start_index).unwrap();
        assert_eq!(data.buffer, buffer);
        assert_eq!(data.start_index, start_index);
    }

    #[test]
    fn test_new_invalid_start_index() {
        let result = ProtoFormatData::new(vec![], 5);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid start index: 5 is greater than buffer length 0"
        );
    }

    #[test]
    fn test_get_data_empty_buffer() {
        let data = ProtoFormatData::new(vec![], 0).unwrap();
        let expected: &[u8] = &[];
        assert_eq!(data.get_data(), expected);
    }

    #[test]
    fn test_get_data_start_index_zero() {
        let buffer = vec![1, 2, 3, 4, 5];
        let data = ProtoFormatData::new(buffer.clone(), 0).unwrap();
        assert_eq!(data.get_data(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_get_data_with_start_index() {
        let buffer = vec![1, 2, 3, 4, 5];
        let start_index = 2;
        let data = ProtoFormatData::new(buffer.clone(), start_index).unwrap();
        assert_eq!(data.get_data(), &[3, 4, 5]);
    }

    #[test]
    fn test_get_data_with_start_index_at_end() {
        let buffer = vec![1, 2, 3, 4, 5];
        let start_index = 5;
        let data = ProtoFormatData::new(buffer.clone(), start_index).unwrap();
        let expected: &[u8] = &[];
        assert_eq!(data.get_data(), expected);
    }
}
