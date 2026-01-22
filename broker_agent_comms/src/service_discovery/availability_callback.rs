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

use async_trait::async_trait;
use dashmap::DashMap;
use log::{error, info};

use google_sdv_someip::{
    aidl::google::sdv::someip::{
        AvailabilityStatus::AvailabilityStatus,
        ISomeIpServiceAvailabilityCallback::ISomeIpServiceAvailabilityCallbackAsyncServer,
        SomeIpService::SomeIpService,
    },
    binder::{self, StatusCode},
};

use crate::agent::{
    protocol_mapping::SomeIpServiceInstanceInfo, service_matcher::SomeIpServiceSet,
    AvailabilityMessage, IAvailabilityNotifier, ServiceType,
};

/// Implementation of the AIDL callback interface `ISomeIpServiceAvailabilityCallback`.
/// Defines processing logic for SOME/IP service availability change.
pub(crate) struct AvailabilityCallback {
    pub services: SomeIpServiceSet,
    pub discovered_someip_services: DashMap<SomeIpService, SomeIpService>,
    pub availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
}

impl AvailabilityCallback {
    pub fn create(
        someip_services: Vec<SomeIpServiceInstanceInfo>,
        availability_notifiers: Vec<Box<dyn IAvailabilityNotifier>>,
    ) -> Self {
        let mut services = SomeIpServiceSet::new();
        for service_instance_info in someip_services {
            services.insert(
                service_instance_info.someip_service_instance,
                service_instance_info.sd_minor_version_mode,
            );
        }
        Self { services, discovered_someip_services: DashMap::new(), availability_notifiers }
    }
}

impl binder::Interface for AvailabilityCallback {}

#[async_trait]
impl ISomeIpServiceAvailabilityCallbackAsyncServer for AvailabilityCallback {
    async fn onAvailabilityChanged(
        &self,
        service: &SomeIpService,
        availability: AvailabilityStatus,
    ) -> binder::Result<()> {
        let Some(mapping_service) = self.services.get_mapping_service(service) else {
            info!(
                "Received service availability notification with no known mapping, Service ID = {}",
                service.service_id
            );
            return Err(StatusCode::NAME_NOT_FOUND.into());
        };
        let available = match availability {
            AvailabilityStatus::AVAILABLE => {
                // we need to make sure that we haven't already offered this service
                if self.discovered_someip_services.contains_key(&mapping_service) {
                    // either the same service sent an available notification twice, or another
                    // service has already matched the mapping criteria
                    return Err(StatusCode::ALREADY_EXISTS.into());
                }
                self.discovered_someip_services.insert(mapping_service, *service);
                true
            }
            AvailabilityStatus::UNAVAILABLE => {
                let should_process_notification = self
                    .discovered_someip_services
                    .get(&mapping_service)
                    .map(|discovered_service_entry| discovered_service_entry.value() == service)
                    .unwrap_or(false);
                if should_process_notification {
                    self.discovered_someip_services.remove(&mapping_service);
                    false
                } else {
                    // We either haven't offered this service, or we've matched the service specified in the mapping to another service
                    return Err(StatusCode::BAD_VALUE.into());
                }
            }
            state => {
                error!("Received unknown service state! (={state:?})");
                return Err(StatusCode::UNKNOWN_ERROR.into());
            }
        };
        for availability_notifier in &self.availability_notifiers {
            if availability_notifier
                .notify(AvailabilityMessage { service: ServiceType::SOMEIP(*service), available })
                .await
                .is_err()
            {
                error!("Failed to notify listener about availability change of SOME/IP service {service:?}");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::protocol_mapping::SomeIpServiceInstanceInfo;
    use crate::agent::MockIAvailabilityNotifier;
    use anyhow::anyhow;
    use google_sdv_someip::aidl::google::sdv::someip::SomeIpServiceVersion::SomeIpServiceVersion;
    use someip_config::broker_params::SdMinorVersionMode;

    fn create_someip_service(
        service_id: u16,
        instance_id: u16,
        major: u8,
        minor: i32,
    ) -> SomeIpService {
        SomeIpService {
            service_id,
            instance_id,
            version: SomeIpServiceVersion { major: major as i8, minor },
        }
    }

    #[tokio::test]
    async fn test_on_availability_changed_available_success() {
        // GIVEN
        let service_to_map = create_someip_service(1, 1, 1, 0);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_to_map,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );

        let mut mock_notifier = MockIAvailabilityNotifier::new();
        mock_notifier
            .expect_notify()
            .withf(move |msg| {
                msg.available
                    && matches!(&msg.service, ServiceType::SOMEIP(s) if *s == service_to_map)
            })
            .times(1)
            .returning(|_| Ok(()));

        let callback = AvailabilityCallback::create(
            vec![service_instance_info],
            vec![Box::new(mock_notifier)],
        );

        // WHEN
        let result =
            callback.onAvailabilityChanged(&service_to_map, AvailabilityStatus::AVAILABLE).await;

        // THEN
        assert!(result.is_ok());
        assert!(callback.discovered_someip_services.contains_key(&service_to_map));
        assert_eq!(
            callback.discovered_someip_services.get(&service_to_map).unwrap().value(),
            &service_to_map
        );
    }

    #[tokio::test]
    async fn test_on_availability_changed_unknown_service() {
        // GIVEN
        let service_to_map = create_someip_service(1, 1, 1, 0);
        let unknown_service = create_someip_service(2, 1, 1, 0); // Different service_id
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_to_map,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let callback = AvailabilityCallback::create(vec![service_instance_info], vec![]);

        // WHEN
        let result =
            callback.onAvailabilityChanged(&unknown_service, AvailabilityStatus::AVAILABLE).await;

        // THEN
        assert_eq!(result.err(), Some(StatusCode::NAME_NOT_FOUND.into()));
    }

    #[tokio::test]
    async fn test_on_availability_changed_available_twice() {
        // GIVEN
        let service_to_map = create_someip_service(1, 1, 1, 0);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_to_map,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let callback = AvailabilityCallback::create(vec![service_instance_info], vec![]);
        callback
            .onAvailabilityChanged(&service_to_map, AvailabilityStatus::AVAILABLE)
            .await
            .unwrap();

        // WHEN
        let result =
            callback.onAvailabilityChanged(&service_to_map, AvailabilityStatus::AVAILABLE).await;

        // THEN
        assert_eq!(result.err(), Some(StatusCode::ALREADY_EXISTS.into()));
    }

    #[tokio::test]
    async fn test_on_availability_changed_available_from_different_minor_version() {
        // GIVEN
        let service_mapping = create_someip_service(1, 1, 1, 1);
        let first_available_service = create_someip_service(1, 1, 1, 10);
        let second_available_service = create_someip_service(1, 1, 1, 20);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_mapping,
            SdMinorVersionMode::MinimumMinorVersion,
        );

        let callback = AvailabilityCallback::create(vec![service_instance_info], vec![]);
        callback
            .onAvailabilityChanged(&first_available_service, AvailabilityStatus::AVAILABLE)
            .await
            .unwrap();

        // WHEN
        let result = callback
            .onAvailabilityChanged(&second_available_service, AvailabilityStatus::AVAILABLE)
            .await;

        // THEN
        assert_eq!(result.err(), Some(StatusCode::ALREADY_EXISTS.into()));
    }

    #[tokio::test]
    async fn test_on_availability_changed_unavailable_success() {
        // GIVEN
        let service_to_map = create_someip_service(1, 1, 1, 0);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_to_map,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );

        let mut mock_notifier = MockIAvailabilityNotifier::new();
        // Expect one call for AVAILABLE
        mock_notifier.expect_notify().times(1).returning(|_| Ok(()));
        // Expect another for UNAVAILABLE
        mock_notifier
            .expect_notify()
            .withf(move |msg| {
                !msg.available
                    && matches!(&msg.service, ServiceType::SOMEIP(s) if *s == service_to_map)
            })
            .times(1)
            .returning(|_| Ok(()));

        let callback = AvailabilityCallback::create(
            vec![service_instance_info],
            vec![Box::new(mock_notifier)],
        );
        callback
            .onAvailabilityChanged(&service_to_map, AvailabilityStatus::AVAILABLE)
            .await
            .unwrap();

        // WHEN
        let result =
            callback.onAvailabilityChanged(&service_to_map, AvailabilityStatus::UNAVAILABLE).await;

        // THEN
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_on_availability_changed_unavailable_when_not_available() {
        // GIVEN
        let service_to_map = create_someip_service(1, 1, 1, 0);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_to_map,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let callback = AvailabilityCallback::create(vec![service_instance_info], vec![]);

        // WHEN
        let result =
            callback.onAvailabilityChanged(&service_to_map, AvailabilityStatus::UNAVAILABLE).await;

        // THEN
        assert_eq!(result.err(), Some(StatusCode::BAD_VALUE.into()));
    }

    #[tokio::test]
    async fn test_on_availability_changed_unavailable_from_different_instance() {
        // GIVEN
        let service_mapping = create_someip_service(1, 1, 1, 1);
        let available_service = create_someip_service(1, 1, 1, 10);
        let unavailable_service = create_someip_service(1, 1, 1, 20);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_mapping,
            SdMinorVersionMode::MinimumMinorVersion,
        );

        let callback = AvailabilityCallback::create(vec![service_instance_info], vec![]);
        callback
            .onAvailabilityChanged(&available_service, AvailabilityStatus::AVAILABLE)
            .await
            .unwrap();

        // WHEN
        let result = callback
            .onAvailabilityChanged(&unavailable_service, AvailabilityStatus::UNAVAILABLE)
            .await;

        // THEN
        assert_eq!(result.err(), Some(StatusCode::BAD_VALUE.into()));
    }

    #[tokio::test]
    async fn test_on_availability_changed_unknown_status() {
        // GIVEN
        let service_to_map = create_someip_service(1, 1, 1, 0);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_to_map,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );
        let callback = AvailabilityCallback::create(vec![service_instance_info], vec![]);

        // WHEN
        let result = callback.onAvailabilityChanged(&service_to_map, AvailabilityStatus(123)).await; // some unknown status

        // THEN
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_on_availability_changed_notifier_fails() {
        // GIVEN
        let service_to_map = create_someip_service(1, 1, 1, 0);
        let service_instance_info = SomeIpServiceInstanceInfo::new(
            service_to_map,
            SdMinorVersionMode::ExactOrAnyMinorVersion,
        );

        let mut mock_notifier = MockIAvailabilityNotifier::new();
        mock_notifier.expect_notify().times(1).returning(|_| Err(anyhow!("notification failed")));

        let callback = AvailabilityCallback::create(
            vec![service_instance_info],
            vec![Box::new(mock_notifier)],
        );

        // WHEN
        let result =
            callback.onAvailabilityChanged(&service_to_map, AvailabilityStatus::AVAILABLE).await;

        // THEN
        // The operation should succeed even if the notifier fails (it just logs an error)
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_availability_callback() {
        let service1 = create_someip_service(1, 1, 1, 0);
        let service2 = create_someip_service(2, 1, 1, 0);

        let info1 =
            SomeIpServiceInstanceInfo::new(service1, SdMinorVersionMode::ExactOrAnyMinorVersion);
        let info2 =
            SomeIpServiceInstanceInfo::new(service2, SdMinorVersionMode::MinimumMinorVersion);

        let callback = AvailabilityCallback::create(vec![info1, info2], vec![]);

        assert!(callback.services.get_mapping_service(&service1).is_some());
        assert!(callback.services.get_mapping_service(&service2).is_some());
        assert!(callback.availability_notifiers.is_empty());
    }

    #[tokio::test]
    async fn test_on_availability_toggle_different_minor_version_success() {
        // GIVEN
        let service = create_someip_service(1, 1, 1, 10);
        let service_one = create_someip_service(1, 1, 1, 20);
        let service_two = create_someip_service(1, 1, 1, 30);

        let service_instance_info =
            SomeIpServiceInstanceInfo::new(service, SdMinorVersionMode::MinimumMinorVersion);

        let mut mock_notifier = MockIAvailabilityNotifier::new();
        // First expect one call for AVAILABLE
        mock_notifier.expect_notify().times(1).returning(|_| Ok(()));
        // Expect another for UNAVAILABLE
        mock_notifier
            .expect_notify()
            .withf(move |msg| {
                !msg.available
                    && matches!(&msg.service, ServiceType::SOMEIP(s) if *s == service_one)
            })
            .times(1)
            .returning(|_| Ok(()));
        // Then expect another call for AVAILABLE
        mock_notifier.expect_notify().times(1).returning(|_| Ok(()));

        let callback = AvailabilityCallback::create(
            vec![service_instance_info],
            vec![Box::new(mock_notifier)],
        );
        callback.onAvailabilityChanged(&service_one, AvailabilityStatus::AVAILABLE).await.unwrap();

        callback
            .onAvailabilityChanged(&service_one, AvailabilityStatus::UNAVAILABLE)
            .await
            .unwrap();

        // WHEN
        let result =
            callback.onAvailabilityChanged(&service_two, AvailabilityStatus::AVAILABLE).await;

        // THEN
        assert!(result.is_ok());
    }
}
