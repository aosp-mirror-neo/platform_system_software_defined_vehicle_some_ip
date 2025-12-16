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

use std::collections::HashMap;

use google_sdv_someip::aidl::google::sdv::someip::{
    SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
};
use someip_config::broker_params::SdMinorVersionMode;

const ANY_MINOR_VERSION: i32 = 0xFFFFFFFFu32 as i32;

/// This struct is meant to be used by SOME/IP broker tasks as a replacement for
/// HashMap<SomeIpService, T> **when lookups are by actual SOME/IP service IDs detected at
/// runtime**.
/// This is needed because the mapping files may not contain the exact SOME/IP IDs that will be
/// encountered at runtime, so a direct comparison isn't possible for all values.
pub(crate) struct SomeIpServiceMap<T> {
    minimum_or_any_map: HashMap<(u16, u16, i8), (i32, T)>,
    exact_match_map: HashMap<SomeIpService, T>,
}

impl<T> SomeIpServiceMap<T> {
    pub(crate) fn new() -> Self {
        SomeIpServiceMap { exact_match_map: HashMap::new(), minimum_or_any_map: HashMap::new() }
    }

    pub(crate) fn insert(
        &mut self,
        someip_service: SomeIpService,
        sd_minor_version_mode: SdMinorVersionMode,
        value: T,
    ) {
        match sd_minor_version_mode {
            SdMinorVersionMode::ExactOrAnyMinorVersion => {
                if someip_service.version.minor != ANY_MINOR_VERSION {
                    self.exact_match_map.insert(someip_service, value);
                } else {
                    // we store the services specified with ANY_MINOR_VERSION with the services specified
                    // with MinimumMinorVersion mode since we can only have 1 minor version
                    // in the mapping for such services.
                    let key = (
                        someip_service.service_id,
                        someip_service.instance_id,
                        someip_service.version.major,
                    );
                    let value = (someip_service.version.minor, value);
                    self.minimum_or_any_map.insert(key, value);
                }
            }
            SdMinorVersionMode::MinimumMinorVersion => {
                let key = (
                    someip_service.service_id,
                    someip_service.instance_id,
                    someip_service.version.major,
                );
                let value = (someip_service.version.minor, value);
                self.minimum_or_any_map.insert(key, value);
            }
        }
    }

    pub(crate) fn get(&self, someip_service: &SomeIpService) -> Option<&T> {
        let minimum_or_any_key =
            (someip_service.service_id, someip_service.instance_id, someip_service.version.major);
        if let Some((minor_version, value)) = self.minimum_or_any_map.get(&minimum_or_any_key) {
            if *minor_version == ANY_MINOR_VERSION || someip_service.version.minor >= *minor_version
            {
                return Some(value);
            }
        }
        self.exact_match_map.get(someip_service)
    }

    pub(crate) fn get_mapping_service(
        &self,
        someip_service: &SomeIpService,
    ) -> Option<SomeIpService> {
        let minimum_or_any_key =
            (someip_service.service_id, someip_service.instance_id, someip_service.version.major);
        if let Some((minor_version, _)) = self.minimum_or_any_map.get(&minimum_or_any_key) {
            if *minor_version == ANY_MINOR_VERSION || someip_service.version.minor >= *minor_version
            {
                return Some(SomeIpService {
                    service_id: someip_service.service_id,
                    instance_id: someip_service.instance_id,
                    version: SomeIpServiceVersion {
                        major: someip_service.version.major,
                        minor: *minor_version,
                    },
                });
            }
        }
        if self.exact_match_map.contains_key(someip_service) {
            Some(*someip_service)
        } else {
            None
        }
    }
}

pub(crate) struct SomeIpServiceSet {
    internal: SomeIpServiceMap<()>,
}

impl SomeIpServiceSet {
    pub(crate) fn new() -> Self {
        SomeIpServiceSet { internal: SomeIpServiceMap::new() }
    }

    pub(crate) fn insert(
        &mut self,
        someip_service: SomeIpService,
        sd_minor_version_mode: SdMinorVersionMode,
    ) {
        self.internal.insert(someip_service, sd_minor_version_mode, ());
    }

    pub(crate) fn get_mapping_service(
        &self,
        someip_service: &SomeIpService,
    ) -> Option<SomeIpService> {
        self.internal.get_mapping_service(someip_service)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_someip_service(
        service_id: u16,
        instance_id: u16,
        major: i8,
        minor: i32,
    ) -> SomeIpService {
        SomeIpService { service_id, instance_id, version: SomeIpServiceVersion { major, minor } }
    }

    #[test]
    fn test_new_map_is_empty() {
        let map: SomeIpServiceMap<i32> = SomeIpServiceMap::new();
        assert!(map.exact_match_map.is_empty());
        assert!(map.minimum_or_any_map.is_empty());
    }

    #[test]
    fn test_insert_and_get_exact_match() {
        let mut map = SomeIpServiceMap::new();
        let service = create_someip_service(1, 1, 1, 10);
        map.insert(service, SdMinorVersionMode::ExactOrAnyMinorVersion, 100);

        // Exact match
        assert_eq!(map.get(&service), Some(&100));

        // Different minor version
        let service_diff_minor = create_someip_service(1, 1, 1, 11);
        assert_eq!(map.get(&service_diff_minor), None);

        // Different instance id
        let service_diff_instance = create_someip_service(1, 2, 1, 10);
        assert_eq!(map.get(&service_diff_instance), None);
    }

    #[test]
    fn test_insert_and_get_any_minor_version() {
        let mut map = SomeIpServiceMap::new();
        let service_any_minor = create_someip_service(1, 1, 1, ANY_MINOR_VERSION);
        map.insert(service_any_minor, SdMinorVersionMode::ExactOrAnyMinorVersion, 200);

        // Match with any minor version
        assert_eq!(map.get(&create_someip_service(1, 1, 1, 0)), Some(&200));
        assert_eq!(map.get(&create_someip_service(1, 1, 1, 100)), Some(&200));
        assert_eq!(map.get(&create_someip_service(1, 1, 1, ANY_MINOR_VERSION)), Some(&200));

        // Different instance id
        let service_diff_instance = create_someip_service(1, 2, 1, 10);
        assert_eq!(map.get(&service_diff_instance), None);

        // Different major version
        let service_diff_major = create_someip_service(1, 1, 2, 10);
        assert_eq!(map.get(&service_diff_major), None);
    }

    #[test]
    fn test_insert_and_get_minimum_minor_version() {
        let mut map = SomeIpServiceMap::new();
        let service = create_someip_service(1, 1, 1, 10);
        map.insert(service, SdMinorVersionMode::MinimumMinorVersion, 300);

        // Match with same minor version
        assert_eq!(map.get(&create_someip_service(1, 1, 1, 10)), Some(&300));

        // Match with greater minor version
        assert_eq!(map.get(&create_someip_service(1, 1, 1, 11)), Some(&300));

        // No match with smaller minor version
        assert_eq!(map.get(&create_someip_service(1, 1, 1, 9)), None);

        // No match with different instance id
        assert_eq!(map.get(&create_someip_service(1, 2, 1, 10)), None);
    }

    #[test]
    fn test_get_from_empty_map() {
        let map: SomeIpServiceMap<i32> = SomeIpServiceMap::new();
        let service = create_someip_service(1, 1, 1, 1);
        assert_eq!(map.get(&service), None);
    }

    #[test]
    fn test_precedence_of_minimum_or_any_map() {
        let mut map = SomeIpServiceMap::new();

        let service_min = create_someip_service(1, 1, 1, 10);
        map.insert(service_min, SdMinorVersionMode::MinimumMinorVersion, 100);

        let service_exact = create_someip_service(1, 1, 1, 12);
        map.insert(service_exact, SdMinorVersionMode::ExactOrAnyMinorVersion, 200);

        // Lookup with minor version 12. It should match the minimum version rule first.
        let lookup_service = create_someip_service(1, 1, 1, 12);
        assert_eq!(map.get(&lookup_service), Some(&100));
    }

    #[test]
    fn test_get_mapping_service_exact_match() {
        let mut map = SomeIpServiceMap::new();
        let service = create_someip_service(1, 1, 1, 10);
        map.insert(service, SdMinorVersionMode::ExactOrAnyMinorVersion, 100);

        assert_eq!(map.get_mapping_service(&service), Some(service));
    }

    #[test]
    fn test_get_mapping_service_any_minor() {
        let mut map = SomeIpServiceMap::new();
        let service_any_minor = create_someip_service(1, 1, 1, ANY_MINOR_VERSION);
        map.insert(service_any_minor, SdMinorVersionMode::ExactOrAnyMinorVersion, 200);

        let lookup_service = create_someip_service(1, 1, 1, 50);
        assert_eq!(map.get_mapping_service(&lookup_service), Some(service_any_minor));
    }

    #[test]
    fn test_get_mapping_service_minimum_minor() {
        let mut map = SomeIpServiceMap::new();
        let service_min = create_someip_service(1, 1, 1, 10);
        map.insert(service_min, SdMinorVersionMode::MinimumMinorVersion, 300);

        let lookup_service = create_someip_service(1, 1, 1, 50);
        assert_eq!(map.get_mapping_service(&lookup_service), Some(service_min));
    }

    #[test]
    fn test_get_mapping_service_no_match() {
        let map: SomeIpServiceMap<i32> = SomeIpServiceMap::new();
        let service = create_someip_service(1, 1, 1, 1);
        assert_eq!(map.get_mapping_service(&service), None);
    }
}
