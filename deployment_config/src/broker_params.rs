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

//! Config params for SOME/IP broker behavior

use serde::de::{self, Deserialize, Deserializer, Visitor};
use std::fmt;

/// This mode controls the default behavior of the SOME/IP broker when checking for service minor versions during service discovery
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum SdMinorVersionMode {
    /// In this mode, SOME/IP broker will not accept SOME/IP services with minor versions that are different from the ones specified in the SOME/IP mapping, except in the case of the value `0xFFFFFFFF` which corresponds to Any Minor Version. In that case, the broker will accept any matching service disregarding the minor version
    #[default]
    ExactOrAnyMinorVersion = 0,
    /// In this mode, SOME/IP broker will accept any SOME/IP service that has a minor version greater than or equal to the one specified in the SOME/IP mapping
    MinimumMinorVersion = 1,
}

/// Implement custom deserialization for SdMinorVersionMode to be able to deserialize from constant names
impl<'de> Deserialize<'de> for SdMinorVersionMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SdMinorVersionModeVisitor;

        impl Visitor<'_> for SdMinorVersionModeVisitor {
            type Value = SdMinorVersionMode;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("Either ExactOrAnyMinorVersion or MinimumMinorVersion")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                const VARIANTS: &[&str] = &["ExactOrAnyMinorVersion", "MinimumMinorVersion"];
                match v {
                    "ExactOrAnyMinorVersion" => Ok(SdMinorVersionMode::ExactOrAnyMinorVersion),
                    "MinimumMinorVersion" => Ok(SdMinorVersionMode::MinimumMinorVersion),
                    var => Err(de::Error::unknown_variant(var, VARIANTS)),
                }
            }
        }
        deserializer.deserialize_str(SdMinorVersionModeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::SdMinorVersionMode;

    #[test]
    fn test_deserialize_sd_minor_version_mode() {
        let exact: SdMinorVersionMode =
            serde_json::from_str(r#""ExactOrAnyMinorVersion""#).unwrap();
        assert_eq!(exact, SdMinorVersionMode::ExactOrAnyMinorVersion);

        let minimum: SdMinorVersionMode = serde_json::from_str(r#""MinimumMinorVersion""#).unwrap();
        assert_eq!(minimum, SdMinorVersionMode::MinimumMinorVersion);

        let invalid_json = r#""InvalidMode""#;
        let result: Result<SdMinorVersionMode, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("unknown variant `InvalidMode`"));
    }
}
