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
use log::error;

use std::sync::Arc;

use google_sdv_someip::{
    aidl::google::sdv::someip::{
        IEventNotificationCallback::IEventNotificationCallbackAsyncServer,
        SomeIpService::SomeIpService,
    },
    binder::{self, StatusCode},
};

use crate::translator::IBrokerTranslator;
use crate::translator::ProtoFormatData;
use crate::{
    binder_load_indicators::load_indicators::LoadIndicators, pubsub::config::SimplePublisherInfo,
};
use crate::{
    pubsub::config::{BrokerPublisher, SomeIpEventSubscriberInfo},
    translator::SomeIpDescriptorId,
};

use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};

use futures_executor::ThreadPool;
use futures_executor::ThreadPoolBuilder;

const WORKER_THREADS_COUNT: u8 = 4;

/// Implementation of the AIDL callback interface `IEventNotificationCallback`.
pub(crate) struct EventNotificationCallback {
    broker_publisher_map: Arc<DashMap<SomeIpEventSubscriberInfo, BrokerPublisher>>,
    translator: Arc<dyn IBrokerTranslator>,
    threads: Vec<ThreadPool>,
    load_indicators: Option<Arc<LoadIndicators>>,
}

impl binder::Interface for EventNotificationCallback {}

impl EventNotificationCallback {
    pub fn create(
        broker_publisher_map: Arc<DashMap<SomeIpEventSubscriberInfo, BrokerPublisher>>,
        translator: Arc<dyn IBrokerTranslator>,
        load_indicators: Option<Arc<LoadIndicators>>,
    ) -> Self {
        let threads = (0..WORKER_THREADS_COUNT)
            .filter_map(|index| {
                ThreadPoolBuilder::new()
                    .pool_size(1)
                    .name_prefix(format!("someip-onEvent-{index}"))
                    .create()
                    .ok()
            })
            .collect();

        Self { broker_publisher_map, translator, threads, load_indicators }
    }

    #[tracing::instrument(skip_all)]
    async fn publish_to_sdv(
        publisher: &SimplePublisherInfo,
        event_payload: &[u8],
        someip_event: &SomeIpEventSubscriberInfo,
    ) -> binder::Result<()> {
        let _ = publisher.publish(event_payload).await.map_err(|e| {
            error!("Failed to publish message from SOME/IP event {someip_event:?}, {e:?}");
            StatusCode::UNKNOWN_ERROR
        })?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn publish_group_parts(
        translated_group_events: Vec<anyhow::Result<ProtoFormatData>>,
        group_publisher_infos: &[SimplePublisherInfo],
        someip_event: &SomeIpEventSubscriberInfo,
    ) -> binder::Result<()> {
        for (index, publisher_info) in group_publisher_infos.iter().enumerate() {
            if let Ok(message_part) = &translated_group_events[index] {
                Self::publish_to_sdv(publisher_info, message_part.get_data(), someip_event).await?;
            }
        }
        Ok(())
    }

    async fn process_on_event(
        broker_publisher_map: Arc<DashMap<SomeIpEventSubscriberInfo, BrokerPublisher>>,
        translator: Arc<dyn IBrokerTranslator>,
        event_payload: Vec<u8>,
        someip_event: SomeIpEventSubscriberInfo,
        load_indicators: Option<Arc<LoadIndicators>>,
    ) {
        if let Some(load_indicators) = load_indicators {
            load_indicators.queue_decreased();
        }

        let Some(broker_publisher) = broker_publisher_map.get(&someip_event) else {
            error!("Received event with no known mapping, event ID = {}", someip_event.event_id);
            return;
        };

        match broker_publisher.value() {
            BrokerPublisher::SIMPLE(simple_publisher_info) => {
                let someip_event_key = SomeIpDescriptorId::new_event_id(
                    someip_event.service_id,
                    someip_event.version_major,
                    someip_event.event_id,
                );
                match translator.translate_from_someip_to_protobuf_topic(
                    simple_publisher_info.get_unit_type(),
                    &event_payload,
                    &someip_event_key,
                ) {
                    Ok(proto_format_data) => {
                        Self::publish_to_sdv(
                            simple_publisher_info,
                            proto_format_data.get_data(),
                            &someip_event,
                        )
                        .await
                        .unwrap_or_default();
                    }
                    Err(e) => {
                        error!("Error translating SOME/IP message for simple event {someip_event:?} into {}: {e:?}", simple_publisher_info.get_unit_type());
                    }
                }
            }
            BrokerPublisher::GROUP(group_publisher_infos) => {
                let someip_group_event_key = SomeIpDescriptorId::new_event_id(
                    someip_event.service_id,
                    someip_event.version_major,
                    someip_event.event_id,
                );
                match translator
                    .translate_someip_group_event(&event_payload, &someip_group_event_key)
                {
                    Ok(translated_group_events) => {
                        Self::publish_group_parts(
                            translated_group_events,
                            group_publisher_infos,
                            &someip_event,
                        )
                        .await
                        .unwrap_or_default();
                    }
                    Err(e) => {
                        error!("Error deserializing SOME/IP message for group event {someip_event:?} - {e:?}");
                    }
                }
            }
        }
    }
}

#[async_trait]
impl IEventNotificationCallbackAsyncServer for EventNotificationCallback {
    #[tracing::instrument(skip(self, event_payload))]
    async fn onEvent(
        &self,
        someip_service: &SomeIpService,
        event: u16,
        event_payload: &[u8],
    ) -> binder::Result<()> {
        let someip_event = SomeIpEventSubscriberInfo::new(*someip_service, event);

        let mut hasher = DefaultHasher::new();
        someip_event.hash(&mut hasher);
        let hash_value = hasher.finish();

        let Some(thread) = self.threads.get((hash_value % self.threads.len() as u64) as usize)
        else {
            return Ok(());
        };

        if let Some(load_indicators) = self.load_indicators.as_ref() {
            load_indicators.queue_increased();
        }
        thread.spawn_ok(Self::process_on_event(
            self.broker_publisher_map.clone(),
            self.translator.clone(),
            event_payload.to_vec(),
            someip_event,
            self.load_indicators.clone(),
        ));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::MockIBrokerTranslator;

    fn create_test_someip_event_subscriber_info() -> SomeIpEventSubscriberInfo {
        SomeIpEventSubscriberInfo { service_id: 1, instance_id: 1, version_major: 1, event_id: 10 }
    }

    #[tokio::test]
    async fn process_on_event_decrements_load_indicators() {
        // GIVEN
        let broker_publisher_map = Arc::new(DashMap::new());
        let translator = Arc::new(MockIBrokerTranslator::new());
        let event_payload = vec![];
        let someip_event = create_test_someip_event_subscriber_info();
        let load_indicators = Arc::new(LoadIndicators::new());
        load_indicators.queue_increased(); // Set to 1
        assert_eq!(load_indicators.get_current_num_pending_messages(), 1);

        // WHEN
        EventNotificationCallback::process_on_event(
            broker_publisher_map,
            translator,
            event_payload,
            someip_event,
            Some(load_indicators.clone()),
        )
        .await;

        // THEN
        assert_eq!(load_indicators.get_current_num_pending_messages(), 0);
    }

    #[tokio::test]
    async fn process_on_event_with_no_publisher_returns() {
        // GIVEN
        let broker_publisher_map = Arc::new(DashMap::new());
        let mut translator = MockIBrokerTranslator::new();
        let event_payload = vec![1, 2, 3];
        let someip_event = create_test_someip_event_subscriber_info();

        // We expect no calls to translators if no publisher is found.
        translator.expect_translate_from_someip_to_protobuf_topic().times(0);

        // WHEN
        EventNotificationCallback::process_on_event(
            broker_publisher_map,
            Arc::new(translator),
            event_payload,
            someip_event,
            None,
        )
        .await;

        // THEN: No panic, and mocks were not called.
    }
}
