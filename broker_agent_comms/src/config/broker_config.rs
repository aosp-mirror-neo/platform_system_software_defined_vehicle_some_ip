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

//! Config for SOME/IP Broker

use std::path::PathBuf;
use std::{fs, time::Duration};

use serde::{Deserialize, Deserializer};
use thiserror::Error;

use someip_config::broker_params::SdMinorVersionMode;
use someip_config::serde_config::ffi::SomeIpConfig;

/// Broker config contains OEM-specific information including:
/// - Serialization parameters for OEM-specific SOME/IP rules (byte order, size field width, etc).
/// - Frequency for pushing and pulling
#[derive(Clone, Deserialize, Debug)]
pub(crate) struct BrokerConfig {
    /// OEM-specific SOME/IP rules (byte order, size field width, etc).
    pub serialization_config: SomeIpConfig,

    /// Polling interval for SDV DT topics data
    #[serde(deserialize_with = "deserialize_duration", rename = "dt_polling_interval_ms")]
    pub dt_polling_interval: Duration,

    /// Default behavior of the SOME/IP broker when checking for service minor versions during service discovery
    #[serde(default)]
    pub default_sd_minor_version_mode: SdMinorVersionMode,
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let milliseconds = u64::deserialize(deserializer)?;

    Ok(Duration::from_millis(milliseconds))
}

/// Represents errors that can be returned from the get_config function
#[derive(Error, Debug)]
pub(crate) enum BrokerConfigParserError {
    /// Error produced by json parser library
    /// Usually means that input is malformed
    #[error("Json parse error:\n`{0}`")]
    ParseError(#[from] serde_json::Error),

    /// Error trying to read file. It's either does not exist or there are access restrictions
    #[error("Filesystem error:\n`{0}`")]
    FilesystemError(#[from] std::io::Error),
}

impl BrokerConfig {
    fn parse(json_string: &str) -> Result<Self, Box<BrokerConfigParserError>> {
        serde_json::from_str(json_string)
            .map_err(|err| Box::new(BrokerConfigParserError::from(err)))
    }

    /// Reads and parses the config from file.
    pub(crate) fn get_config(config_path: PathBuf) -> Result<Self, Box<BrokerConfigParserError>> {
        BrokerConfig::parse(
            &fs::read_to_string(config_path)
                .map_err(|err| Box::new(BrokerConfigParserError::from(err)))?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use someip_config::broker_params::SdMinorVersionMode;
    use someip_config::serde_config::ffi::Endianness;

    #[googletest::test]
    fn test_valid_config_with_missing_sd_minor_version_mode() {
        let json = r#"
        {
            "serialization_config": {
                "endianness": "BigEndian",
                "string_type": "UTF_8",
                "len_field_size": "TwoBytes"
            },
            "dt_polling_interval_ms": 123
        }"#;
        let config = BrokerConfig::parse(json).unwrap();
        expect_that!(config.serialization_config.endianness, eq(Endianness::BigEndian));
        expect_that!(config.dt_polling_interval, eq(Duration::from_millis(123)));
        expect_that!(
            config.default_sd_minor_version_mode,
            eq(SdMinorVersionMode::ExactOrAnyMinorVersion)
        );
    }

    #[googletest::test]
    fn test_valid_config_with_sd_minor_version_mode() {
        let json = r#"
        {
            "serialization_config": {
                "endianness": "BigEndian",
                "string_type": "UTF_8",
                "len_field_size": "TwoBytes"
            },
            "dt_polling_interval_ms": 123,
            "default_sd_minor_version_mode": "MinimumMinorVersion"
        }"#;
        let config = BrokerConfig::parse(json).unwrap();
        expect_that!(config.serialization_config.endianness, eq(Endianness::BigEndian));
        expect_that!(config.dt_polling_interval, eq(Duration::from_millis(123)));
        expect_that!(
            config.default_sd_minor_version_mode,
            eq(SdMinorVersionMode::MinimumMinorVersion)
        );
    }

    #[googletest::test]
    fn test_invalid_json_format() {
        let invalid_json = r#"
        {
            // Missing a closing brace!
        "#;

        let result = BrokerConfig::parse(invalid_json);
        assert!(result.is_err());
    }

    #[googletest::test]
    fn test_missing_fields() {
        let missing_field_json = r#"
        {
            "serialization_config": {
                "endianness": "BigEndian",
                "string_type": "UTF_8",
                "len_field_size": "TwoBytes"
            }
        }"#;

        let result = BrokerConfig::parse(missing_field_json);
        assert!(result.is_err());
    }

    #[googletest::test]
    fn test_deserialize_duration_error() {
        let invalid_duration_json = r#"({
            "serialization_config": {
                "endianness": "BigEndian",
                "string_type": "UTF_8",
                "len_field_size": "TwoBytes"
            },
            "dt_polling_interval_ms": 123.3
        }"#;
        let invalid_duration_json = invalid_duration_json.to_string();

        let result = BrokerConfig::parse(&invalid_duration_json);
        assert!(result.is_err());
    }
}
