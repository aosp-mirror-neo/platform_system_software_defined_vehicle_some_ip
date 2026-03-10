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
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::bail;
use async_trait::async_trait;
use dashmap::DashMap;
use log::warn;
use mockall::automock;
use tokio::sync::Mutex;
use tokio::sync::{MappedMutexGuard, MutexGuard};
use tokio::time::{sleep, Duration, Instant};

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;

use sdv_comms::dt::Subscriber;
use sdv_comms::id::ServiceFqin;
use sdv_comms::sd::{
    PublisherUnitMetadata, ServiceUnitDefinition, UnitMetadata, UnitRegistrationArgs, UnitType,
};

use sdv_vsidl_common::proto_size_utils::compute_message_length_including_header;

use crate::agent::protocol_mapping::SdvTopicMappingInfo;
use crate::agent::protocol_mapping::SomeIpEvent;
use crate::agent::protocol_mapping::SomeIpEventMappingInfo;
use crate::agent::protocol_mapping::{BrokerServiceUnit, SdvServiceUnitCommsId, SdvServiceUnitId};
use crate::agent::protocol_mapping::{SdvServiceUnitInfo, SomeIpServiceInstanceInfo};
use crate::agent::service_matcher::SomeIpServiceMap;
use crate::agent::IAvailabilitySubscriber;
use crate::agent::ServiceType;
use crate::binder_load_indicators::load_indicators::LoadIndicators;
use crate::comms::ISdvCommsContext;
use crate::comms::ISomeIpStackContext;
use crate::pubsub::config::{
    BrokerMultipubSubscriberInfo, BrokerPublisher, BrokerSubscriberInfo, BrokerSubscriberType,
    PollableSubscriber, PubSubEventMappingInfo, PubSubEventMappingType, PubSubTopicInfo,
    SdvMultipubTopicInfo, SimplePublisherInfo, SomeIpEventSubscriberInfo,
};
use crate::pubsub::EventNotificationCallback;
use crate::translator::IBrokerTranslator;
use crate::translator::TranslationMode;

#[automock]
#[async_trait]
pub(crate) trait IPubSubHandler: Send + Sync {
    fn add_topic_mapping(
        &mut self,
        service_unit_info: SdvServiceUnitInfo,
        someip_service_instance_info: SomeIpService,
        topic_mapping_info: SdvTopicMappingInfo,
    );
    fn add_multpub_topic_mapping(
        &mut self,
        unit_type: UnitType,
        someip_service_instance_info: SomeIpService,
        someip_eventgroup: u16,
        someip_event: u16,
    );
    fn add_event_mapping(
        &mut self,
        someip_service_instance_info: SomeIpServiceInstanceInfo,
        event_mapping_info: Vec<SomeIpEventMappingInfo>,
    );
    fn create_event_notification_callback(
        &self,
        load_indicators: Option<Arc<LoadIndicators>>,
    ) -> EventNotificationCallback;

    async fn start(
        &self,
        mut availability_subscriber: Box<dyn IAvailabilitySubscriber>,
        someip_comms: Arc<dyn ISomeIpStackContext>,
    ) -> anyhow::Result<()>;
}

pub(crate) struct PubSubHandler {
    sdv_comms: Arc<dyn ISdvCommsContext>,
    translator: Arc<dyn IBrokerTranslator>,
    dt_polling_interval: Duration,
    broker_publisher_map: Arc<DashMap<SomeIpEventSubscriberInfo, BrokerPublisher>>,
    sdv_service_mappings: HashMap<SdvServiceUnitId, PubSubTopicInfo>,
    sdv_multipub_mappings: HashMap<UnitType, SdvMultipubTopicInfo>,
    someip_service_mappings: SomeIpServiceMap<Vec<PubSubEventMappingInfo>>,
}

impl PubSubHandler {
    pub(crate) fn new(
        sdv_comms: Arc<dyn ISdvCommsContext>,
        translator: Arc<dyn IBrokerTranslator>,
        dt_polling_interval: Duration,
    ) -> Self {
        Self {
            sdv_comms,
            translator,
            dt_polling_interval,
            broker_publisher_map: Arc::new(DashMap::new()),
            sdv_service_mappings: HashMap::new(),
            sdv_multipub_mappings: HashMap::new(),
            someip_service_mappings: SomeIpServiceMap::new(),
        }
    }
}

#[async_trait]
impl IPubSubHandler for PubSubHandler {
    fn add_topic_mapping(
        &mut self,
        service_unit_info: SdvServiceUnitInfo,
        someip_service_instance_info: SomeIpService,
        topic_mapping_info: SdvTopicMappingInfo,
    ) {
        self.sdv_service_mappings.insert(
            service_unit_info.service_unit_id,
            PubSubTopicInfo::new(someip_service_instance_info, topic_mapping_info),
        );
    }

    fn add_multpub_topic_mapping(
        &mut self,
        unit_type: UnitType,
        someip_service_instance_info: SomeIpService,
        someip_eventgroup: u16,
        someip_event: u16,
    ) {
        self.sdv_multipub_mappings.insert(
            unit_type,
            SdvMultipubTopicInfo::new(
                someip_service_instance_info,
                someip_eventgroup,
                someip_event,
            ),
        );
    }

    fn add_event_mapping(
        &mut self,
        someip_service_instance_info: SomeIpServiceInstanceInfo,
        event_mapping_info: Vec<SomeIpEventMappingInfo>,
    ) {
        self.someip_service_mappings.insert(
            someip_service_instance_info.someip_service_instance,
            someip_service_instance_info.sd_minor_version_mode,
            event_mapping_info.into_iter().map(|e| e.into()).collect(),
        );
    }

    fn create_event_notification_callback(
        &self,
        load_indicators: Option<Arc<LoadIndicators>>,
    ) -> EventNotificationCallback {
        EventNotificationCallback::create(
            self.broker_publisher_map.clone(),
            self.translator.clone(),
            load_indicators.clone(),
        )
    }

    async fn start(
        &self,
        mut availability_subscriber: Box<dyn IAvailabilitySubscriber>,
        someip_comms: Arc<dyn ISomeIpStackContext>,
    ) -> anyhow::Result<()> {
        let broker_subscriber_map = Arc::new(DashMap::new());
        let broker_subscriber_map_clone = broker_subscriber_map.clone();
        let broker_multipub_subscriber_map = Arc::new(DashMap::new());
        let broker_multipub_subscriber_map_clone = broker_multipub_subscriber_map.clone();
        let translator_clone = self.translator.clone();
        let someip_comms_clone = someip_comms.clone();
        let sdv_comms_clone = self.sdv_comms.clone();
        let polling_interval = self.dt_polling_interval;
        let _topic_callback_handle = tokio::spawn(async move {
            loop {
                let poll_start = Instant::now();

                Self::poll_publications(
                    &sdv_comms_clone,
                    &broker_subscriber_map_clone,
                    &broker_multipub_subscriber_map_clone,
                    &translator_clone,
                    &someip_comms_clone,
                )
                .await;

                // Poling should be done with the period equal to `polling_interval`.
                // If events was processed faster than that we sleep for the remaining time.
                //
                // Please note that sleep may take longer than requested depending on scheduler.
                let elapsed = poll_start.elapsed();
                if elapsed < polling_interval {
                    sleep(polling_interval - elapsed).await;
                } else {
                    warn!("Polling publication took {elapsed:?} that is more than the polling interval {polling_interval:?}");
                }
            }
        });

        while let Some(availability_message) = availability_subscriber.receive().await {
            match availability_message.service {
                ServiceType::SDV(service_unit_definition) => {
                    self.handle_sdv_unit_availability(
                        &someip_comms,
                        service_unit_definition,
                        availability_message.available,
                        &broker_subscriber_map,
                        &broker_multipub_subscriber_map,
                    )
                    .await;
                }
                ServiceType::SOMEIP(someip_service) => {
                    let Some(event_mapping_infos) =
                        self.someip_service_mappings.get(&someip_service)
                    else {
                        // service has no events to handle
                        continue;
                    };
                    let someip_comms_clone = Arc::clone(&someip_comms);
                    let event_mapping_infos_clone = event_mapping_infos.clone();
                    let broker_publisher_map_clone = self.broker_publisher_map.clone();
                    let sdv_comms_clone = Arc::clone(&self.sdv_comms);
                    let translator_clone = Arc::clone(&self.translator);
                    tokio::spawn(async move {
                        Self::handle_someip_service_availability(
                            someip_comms_clone,
                            someip_service,
                            availability_message.available,
                            event_mapping_infos_clone,
                            broker_publisher_map_clone,
                            sdv_comms_clone,
                            translator_clone,
                        )
                        .await;
                    });
                }
            }
        }
        return Err(anyhow::Error::msg("Availability subscriber channel has been closed!"));
    }
}

impl PubSubHandler {
    fn find_matching_info(
        &self,
        service_unit_definition: &ServiceUnitDefinition,
    ) -> Option<&PubSubTopicInfo> {
        for (sdv_service_unit, topic_info) in &self.sdv_service_mappings {
            if sdv_service_unit.service_unit_name
                == service_unit_definition.get_unit_identifier().get_unit_name()
                && sdv_service_unit
                    .service_fqin
                    .matches(service_unit_definition.get_service_bundle_identity().get_fqin())
            {
                return Some(topic_info);
            }
        }
        None
    }

    #[tracing::instrument(skip(
        self,
        someip_comms,
        service_unit_definition,
        broker_subscriber_map,
        broker_multipub_subscriber_map
    ))]
    async fn handle_sdv_unit_availability(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        service_unit_definition: Box<ServiceUnitDefinition>,
        available: bool,
        broker_subscriber_map: &Arc<DashMap<SdvServiceUnitCommsId, BrokerSubscriberInfo>>,
        broker_multipub_subscriber_map: &Arc<DashMap<UnitType, BrokerMultipubSubscriberInfo>>,
    ) {
        if let Some(multipub_topic_info) =
            self.sdv_multipub_mappings.get(&service_unit_definition.get_unit_type())
        {
            self.handle_sdv_multipub_topic(
                someip_comms,
                service_unit_definition,
                available,
                broker_multipub_subscriber_map,
                multipub_topic_info,
            )
            .await;
        } else if let Some(pubsub_topic_info) = self.find_matching_info(&service_unit_definition) {
            self.handle_sdv_topic(
                someip_comms,
                service_unit_definition,
                available,
                broker_subscriber_map,
                pubsub_topic_info,
            )
            .await;
        } else {
            // service has no topics to handle
            return;
        };
    }

    #[tracing::instrument(skip(
        self,
        someip_comms,
        service_unit_definition,
        broker_subscriber_map,
        pubsub_topic_info
    ))]
    async fn handle_sdv_topic(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        service_unit_definition: Box<ServiceUnitDefinition>,
        available: bool,
        broker_subscriber_map: &Arc<DashMap<SdvServiceUnitCommsId, BrokerSubscriberInfo>>,
        pubsub_topic_info: &PubSubTopicInfo,
    ) {
        if available {
            self.handle_sdv_topic_mapping_info(
                someip_comms,
                service_unit_definition,
                pubsub_topic_info,
                broker_subscriber_map,
            )
            .await;
        } else {
            if let SdvTopicMappingInfo::SimpleMapping { someip_event_id, someip_eventgroup_id } =
                pubsub_topic_info.sdv_topic_mapping_info
            {
                self.stop_offer_sdv_topic(
                    someip_comms,
                    &pubsub_topic_info.someip_service,
                    someip_eventgroup_id,
                    someip_event_id,
                )
                .await;
            };

            // Subscriber drop impl should be called here
            let _ = broker_subscriber_map.remove(
                &SdvServiceUnitCommsId::from_service_unit_definition(&service_unit_definition),
            );
        }
    }

    #[tracing::instrument(skip_all)]
    async fn handle_sdv_topic_mapping_info(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        service_unit_definition: Box<ServiceUnitDefinition>,
        pubsub_topic_info: &PubSubTopicInfo,
        broker_subscriber_map: &Arc<DashMap<SdvServiceUnitCommsId, BrokerSubscriberInfo>>,
    ) {
        let message_size = if let UnitMetadata::Publisher(metadata) =
            service_unit_definition.get_unit_metadata()
        {
            metadata.get_message_size()
        } else {
            log::error!(
                "Unit type {} is not a publisher, skipping",
                service_unit_definition.get_unit_type()
            );
            return;
        };

        let Ok(message_size) = usize::try_from(message_size) else {
            log::error!(
                "Invalid message size {} for unit type {}, skipping",
                message_size,
                service_unit_definition.get_unit_type()
            );
            return;
        };

        let subscriber = match self
            .sdv_comms
            .create_subscriber(&service_unit_definition.get_unit_identifier())
            .await
        {
            Ok(subscriber) => Some(subscriber),
            Err(e) => {
                log::error!(
                    "Failed to create subscriber for unit type {}: {e:?}, will retry on next poll.",
                    service_unit_definition.get_unit_type()
                );
                None
            }
        };

        let mapping_type = match &pubsub_topic_info.sdv_topic_mapping_info {
            SdvTopicMappingInfo::SimpleMapping { someip_event_id, someip_eventgroup_id } => {
                self.offer_sdv_topic(
                    someip_comms,
                    &pubsub_topic_info.someip_service,
                    *someip_eventgroup_id,
                    *someip_event_id,
                )
                .await;

                BrokerSubscriberType::SIMPLE(SomeIpEvent::new(
                    pubsub_topic_info.someip_service,
                    *someip_event_id,
                    *someip_eventgroup_id,
                ))
            }
            SdvTopicMappingInfo::FireAndForgetMapping { ff_mapping } => {
                BrokerSubscriberType::FF(ff_mapping.clone())
            }
        };
        let map_key = SdvServiceUnitCommsId::from_service_unit_definition(&service_unit_definition);
        broker_subscriber_map.insert(
            map_key,
            BrokerSubscriberInfo {
                subscriber_type: mapping_type,
                service_unit_definition,
                subscriber: Mutex::new(subscriber),
                message_size,
            },
        );
    }

    #[tracing::instrument(skip(
        self,
        someip_comms,
        service_unit_definition,
        broker_multipub_subscriber_map,
        multipub_topic_info
    ))]
    async fn handle_sdv_multipub_topic(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        service_unit_definition: Box<ServiceUnitDefinition>,
        available: bool,
        broker_multipub_subscriber_map: &Arc<DashMap<UnitType, BrokerMultipubSubscriberInfo>>,
        multipub_topic_info: &SdvMultipubTopicInfo,
    ) {
        if available {
            self.handle_sdv_multipub_topic_mapping_info(
                someip_comms,
                service_unit_definition,
                multipub_topic_info,
                broker_multipub_subscriber_map,
            )
            .await;
        } else {
            let entry =
                broker_multipub_subscriber_map.entry(service_unit_definition.get_unit_type());
            if let dashmap::mapref::entry::Entry::Occupied(mut occupied) = entry {
                let broker_multipub_subscriber_info = occupied.get_mut();
                broker_multipub_subscriber_info.publisher_count -= 1;
                if broker_multipub_subscriber_info.publisher_count == 0 {
                    let _ = occupied.remove_entry();

                    self.stop_offer_sdv_topic(
                        someip_comms,
                        &multipub_topic_info.someip_service,
                        multipub_topic_info.someip_eventgroup_id,
                        multipub_topic_info.someip_event_id,
                    )
                    .await;
                }
            }
        }
    }

    async fn handle_sdv_multipub_topic_mapping_info(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        service_unit_definition: Box<ServiceUnitDefinition>,
        multipub_topic_info: &SdvMultipubTopicInfo,
        broker_multipub_subscriber_map: &Arc<DashMap<UnitType, BrokerMultipubSubscriberInfo>>,
    ) {
        let unit_type = service_unit_definition.get_unit_type();
        if !broker_multipub_subscriber_map.contains_key(&unit_type) {
            let subscriber = match self.sdv_comms.create_multipub_subscriber(&unit_type).await {
                Ok(subscriber) => Some(subscriber),
                Err(e) => {
                    log::error!(
                        "Failed to create multipub subscriber for unit type {unit_type}: {e:?}, will retry on next poll."
                    );
                    None
                }
            };

            let message_size = if let UnitMetadata::Publisher(metadata) =
                service_unit_definition.get_unit_metadata()
            {
                metadata.get_message_size()
            } else {
                log::error!("Unit type {unit_type} is not a publisher, skipping");
                return;
            };
            let Ok(message_size) = usize::try_from(message_size) else {
                log::error!(
                    "Invalid message size {message_size} for unit type {unit_type}, skipping"
                );
                return;
            };
            let broker_multipub_subscriber_info = BrokerMultipubSubscriberInfo {
                someip_event: SomeIpEvent::new(
                    multipub_topic_info.someip_service,
                    multipub_topic_info.someip_event_id,
                    multipub_topic_info.someip_eventgroup_id,
                ),
                first_publisher_fqin: service_unit_definition
                    .get_service_bundle_identity()
                    .get_fqin()
                    .clone(),
                unit_type: unit_type.clone(),
                subscriber: Mutex::new(subscriber),
                message_size,
                publisher_count: 1,
            };
            broker_multipub_subscriber_map.insert(unit_type, broker_multipub_subscriber_info);
            // first publisher of a multipub topic, we can now offer the corresponding SOME/IP service
            self.offer_sdv_topic(
                someip_comms,
                &multipub_topic_info.someip_service,
                multipub_topic_info.someip_eventgroup_id,
                multipub_topic_info.someip_event_id,
            )
            .await;
        } else {
            let Some(mut broker_multipub_subscriber_info) =
                broker_multipub_subscriber_map.get_mut(&unit_type)
            else {
                // impossible
                return;
            };
            broker_multipub_subscriber_info.value_mut().publisher_count += 1;
        }
    }

    async fn offer_sdv_topic(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        someip_service: &SomeIpService,
        someip_eventgroup: u16,
        someip_event: u16,
    ) {
        if let Err(e) = someip_comms.offer_service(someip_service).await {
            log::error!("{e:?}");
        }
        if let Err(e) =
            someip_comms.offer_sdv_topic(someip_service, someip_eventgroup, someip_event).await
        {
            log::error!("{e:?}");
        }
    }

    async fn stop_offer_sdv_topic(
        &self,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        someip_service: &SomeIpService,
        someip_eventgroup: u16,
        someip_event: u16,
    ) {
        if let Err(e) = someip_comms.stop_offer_service(someip_service).await {
            log::error!("{e:?}");
        }
        if let Err(e) =
            someip_comms.stop_offer_sdv_topic(someip_service, someip_eventgroup, someip_event).await
        {
            log::error!("{e:?}");
        }
    }

    #[tracing::instrument(skip(someip_comms, broker_publisher_map, sdv_comms, translator))]
    async fn handle_someip_service_availability(
        someip_comms: Arc<dyn ISomeIpStackContext>,
        someip_service: SomeIpService,
        available: bool,
        event_mapping_infos: Vec<PubSubEventMappingInfo>,
        broker_publisher_map: Arc<DashMap<SomeIpEventSubscriberInfo, BrokerPublisher>>,
        sdv_comms: Arc<dyn ISdvCommsContext>,
        translator: Arc<dyn IBrokerTranslator>,
    ) {
        if available {
            let mut event_infos_by_group_id: HashMap<u16, Vec<&PubSubEventMappingInfo>> =
                HashMap::new();
            for event_mapping_info in &event_mapping_infos {
                event_infos_by_group_id
                    .entry(event_mapping_info.eventgroup_id)
                    .or_default()
                    .push(event_mapping_info);
            }
            Self::handle_someip_event_mapping_infos(
                &someip_comms,
                &someip_service,
                &event_infos_by_group_id,
                &broker_publisher_map,
                &sdv_comms,
                &translator,
            )
            .await;
        } else {
            let mut events_by_group_id: HashMap<u16, Vec<u16>> = HashMap::new();
            for event_mapping_info in &event_mapping_infos {
                events_by_group_id
                    .entry(event_mapping_info.eventgroup_id)
                    .or_default()
                    .push(event_mapping_info.event_id);
            }
            for (eventgroup_id, event_ids) in events_by_group_id {
                if let Err(e) = someip_comms
                    .unsubscribe_eventgroup(&someip_service, eventgroup_id, &event_ids)
                    .await
                {
                    log::error!("{e:?}");
                }
                for event in event_ids {
                    // Publisher drop impl should be called here
                    let _ = broker_publisher_map
                        .remove(&SomeIpEventSubscriberInfo::new(someip_service, event));
                }
            }

            // We keep calling the old unsubscribe API to maintain backward compatibility
            for event_mapping_info in event_mapping_infos {
                if let Err(e) = someip_comms
                    .unsubscribe(
                        &someip_service,
                        event_mapping_info.eventgroup_id,
                        event_mapping_info.event_id,
                    )
                    .await
                {
                    log::error!("{e:?}");
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    async fn handle_someip_event_mapping_info(
        someip_service: &SomeIpService,
        event_mapping_info: &PubSubEventMappingInfo,
        broker_publisher_map: &Arc<DashMap<SomeIpEventSubscriberInfo, BrokerPublisher>>,
        sdv_comms: &Arc<dyn ISdvCommsContext>,
        translator: &Arc<dyn IBrokerTranslator>,
    ) -> bool {
        let event_id = event_mapping_info.event_id;

        match &event_mapping_info.event_mapping_type {
            PubSubEventMappingType::SIMPLE(broker_service_unit) => {
                match Self::create_sdv_publisher(broker_service_unit, sdv_comms, translator).await {
                    Ok(simple_publisher_info) => {
                        broker_publisher_map.insert(
                            SomeIpEventSubscriberInfo::new(*someip_service, event_id),
                            BrokerPublisher::SIMPLE(Box::new(simple_publisher_info)),
                        );
                    }
                    Err(e) => {
                        log::error!("{e:?}");
                        return false;
                    }
                }
            }
            PubSubEventMappingType::GROUP(broker_service_units) => {
                let mut all_publishers_creation_success = true;
                let mut publisher_infos = Vec::new();
                for broker_service_unit in broker_service_units {
                    match Self::create_sdv_publisher(broker_service_unit, sdv_comms, translator)
                        .await
                    {
                        Ok(event_info) => publisher_infos.push(event_info),
                        Err(e) => {
                            log::error!("{e:?}");
                            all_publishers_creation_success = false;
                            break;
                        }
                    }
                }
                if all_publishers_creation_success {
                    broker_publisher_map.insert(
                        SomeIpEventSubscriberInfo::new(*someip_service, event_id),
                        BrokerPublisher::GROUP(publisher_infos),
                    );
                } else {
                    return false;
                }
            }
        }
        true
    }

    #[tracing::instrument(skip_all)]
    async fn handle_someip_event_mapping_infos(
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        someip_service: &SomeIpService,
        event_infos_by_group_id: &HashMap<u16, Vec<&PubSubEventMappingInfo>>,
        broker_publisher_map: &Arc<DashMap<SomeIpEventSubscriberInfo, BrokerPublisher>>,
        sdv_comms: &Arc<dyn ISdvCommsContext>,
        translator: &Arc<dyn IBrokerTranslator>,
    ) {
        for (eventgroup_id, event_infos) in event_infos_by_group_id {
            let mut event_ids = Vec::new();
            for event_info in event_infos {
                if Self::handle_someip_event_mapping_info(
                    someip_service,
                    event_info,
                    broker_publisher_map,
                    sdv_comms,
                    translator,
                )
                .await
                {
                    event_ids.push(event_info.event_id);
                }
            }
            if event_ids.is_empty() {
                continue;
            }

            if let Err(e) =
                someip_comms.subscribe_eventgroup(someip_service, *eventgroup_id, &event_ids).await
            {
                log::error!("{e:?}");
            }
            // We keep calling the old subscribe API to maintain backward compatibility
            for event in event_ids {
                if let Err(e) = someip_comms.subscribe(someip_service, *eventgroup_id, event).await
                {
                    log::error!("{e:?}");
                }
            }
        }
    }

    async fn create_sdv_publisher(
        broker_service_unit: &BrokerServiceUnit,
        sdv_comms: &Arc<dyn ISdvCommsContext>,
        translator: &Arc<dyn IBrokerTranslator>,
    ) -> anyhow::Result<SimplePublisherInfo> {
        let mut unit_registration_args = UnitRegistrationArgs::new();
        {
            let mut publisher_unit_metadata = PublisherUnitMetadata::new();
            publisher_unit_metadata.set_version(1);

            let broker_fqin = sdv_comms.get_fqin().await;

            let Ok((message_size, message_count)) = translator
                .get_publication_size_metadata(&broker_fqin, &broker_service_unit.unit_type)
                .await
            else {
                return Err(anyhow::Error::msg(format!(
                    "Couldn't get publication size metadata for unit type {}",
                    broker_service_unit.unit_type
                )));
            };
            publisher_unit_metadata
                .set_message_size(compute_message_length_including_header(message_size));
            publisher_unit_metadata.set_message_count(message_count);

            unit_registration_args.set_unit_name(&broker_service_unit.service_unit_name);
            unit_registration_args.set_unit_type(&broker_service_unit.unit_type);
            unit_registration_args
                .set_unit_metadata(&UnitMetadata::Publisher(publisher_unit_metadata));
        }

        let publisher = Mutex::new(sdv_comms.create_publisher(unit_registration_args).await?);
        Ok(SimplePublisherInfo { publisher, unit_type: broker_service_unit.unit_type.clone() })
    }

    #[tracing::instrument(skip_all)]
    async fn poll_publications(
        sdv_comms: &Arc<dyn ISdvCommsContext>,
        broker_subscriber_map: &Arc<DashMap<SdvServiceUnitCommsId, BrokerSubscriberInfo>>,
        broker_multipub_subscriber_map: &Arc<DashMap<UnitType, BrokerMultipubSubscriberInfo>>,
        translator: &Arc<dyn IBrokerTranslator>,
        someip_comms: &Arc<dyn ISomeIpStackContext>,
    ) {
        // DashMap may deadlock if we hold mutable reference into it in the same thread while
        // iterating. We do obtain one when adding or removing subscribers in the other task, but it should
        // be able to continue in the other Tokio thread even if we hold a reference during await.
        for entry in broker_subscriber_map.iter() {
            let subscriber_info = entry.value();
            Self::poll_single_publication(
                someip_comms,
                sdv_comms,
                translator,
                PollableSubscriber::Single(subscriber_info),
            )
            .await;
        }

        for entry in broker_multipub_subscriber_map.iter() {
            let multipub_subscriber_info = entry.value();
            Self::poll_single_publication(
                someip_comms,
                sdv_comms,
                translator,
                PollableSubscriber::MultiPub(multipub_subscriber_info),
            )
            .await;
        }
    }

    async fn get_or_create_subscriber<'a>(
        sdv_comms: &Arc<dyn ISdvCommsContext>,
        subscriber_info: &PollableSubscriber<'a>,
    ) -> anyhow::Result<MappedMutexGuard<'a, Subscriber>> {
        let subscriber = match subscriber_info {
            PollableSubscriber::Single(info) => &info.subscriber,
            PollableSubscriber::MultiPub(info) => &info.subscriber,
        };
        let mut guarded_subscriber = subscriber.lock().await;

        if guarded_subscriber.is_none() {
            let new_subscriber_result = match subscriber_info {
                PollableSubscriber::Single(info) => {
                    sdv_comms
                        .create_subscriber(&info.service_unit_definition.get_unit_identifier())
                        .await
                }
                PollableSubscriber::MultiPub(info) => {
                    sdv_comms.create_multipub_subscriber(&info.unit_type).await
                }
            };

            match new_subscriber_result {
                Ok(new_subscriber) => {
                    *guarded_subscriber = Some(new_subscriber);
                }
                Err(e) => {
                    let unit_type = match subscriber_info {
                        PollableSubscriber::Single(info) => {
                            info.service_unit_definition.get_unit_type()
                        }
                        PollableSubscriber::MultiPub(info) => info.unit_type.clone(),
                    };

                    bail!("Failed to create subscriber for unit type {unit_type}: {e:?}, will retry on next poll.");
                }
            }
        }

        // We can safely unwrap optional as we just successfully populated it.
        Ok(MutexGuard::map(guarded_subscriber, |s| s.as_mut().unwrap()))
    }

    async fn poll_single_publication(
        someip_comms: &Arc<dyn ISomeIpStackContext>,
        sdv_comms: &Arc<dyn ISdvCommsContext>,
        translator: &Arc<dyn IBrokerTranslator>,
        subscriber_info: PollableSubscriber<'_>,
    ) {
        let guarded_subscriber =
            match Self::get_or_create_subscriber(sdv_comms, &subscriber_info).await {
                Ok(subscriber) => subscriber,
                Err(e) => {
                    log::error!("{e:?}");
                    return;
                }
            };

        let Ok(available_messages) = guarded_subscriber.available_to_read() else {
            log::error!("Couldn't get number of available messages, skipping");
            return;
        };

        if available_messages == 0 {
            // no messages to read
            return;
        }

        let message_size = subscriber_info.message_size();

        // We can allocate a max buffer size of usize::max
        let buffer_size_container = (message_size as u64) * (available_messages as u64);
        let buffer_size = usize::try_from(buffer_size_container).unwrap_or(usize::MAX);
        let mut buffer: Vec<u8> = vec![0; buffer_size];
        let read_result = guarded_subscriber.read_messages(&mut buffer);

        drop(guarded_subscriber);

        match read_result {
            Ok(0) => {}
            Ok(_) => {
                let messages_in_bytes: Vec<Box<[u8]>> = buffer
                    .chunks(message_size)
                    .map(|chunk| chunk.to_owned().into_boxed_slice())
                    .collect();

                for message in messages_in_bytes.into_iter() {
                    if let Err(e) = Self::process_message(
                        translator,
                        &subscriber_info.get_fqin(),
                        &subscriber_info.get_unit_type(),
                        &subscriber_info.get_subscriber_type(),
                        &message,
                        someip_comms,
                    )
                    .await
                    {
                        log::error!("{e:?}");
                    }
                }
            }

            Err(result) => {
                log::error!(
                    "Could not read message from publication with unit type {0} : Error = {1:?}",
                    subscriber_info.get_unit_type(),
                    result
                )
            }
        }
    }

    #[tracing::instrument(skip_all, fields(
        unit_type_name = unit_type.get_unit_type_name(),
        await_start_timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_else(
            |e| e.duration()
        ).as_nanos()
    ))]
    async fn process_message(
        translator: &Arc<dyn IBrokerTranslator>,
        service_fqin: &ServiceFqin,
        unit_type: &UnitType,
        subscriber_type: &BrokerSubscriberType,
        message: &[u8],
        someip_comms: &Arc<dyn ISomeIpStackContext>,
    ) -> anyhow::Result<()> {
        let someip_bytes = translator
            .translate_from_protobuf_to_someip(
                service_fqin,
                unit_type,
                message,
                TranslationMode::Topic,
            )
            .await?;
        match subscriber_type {
            BrokerSubscriberType::SIMPLE(event) => {
                someip_comms.publish(&event.someip_service, event.event_id, someip_bytes).await
            }
            BrokerSubscriberType::FF(someip_methods) => {
                for someip_method in someip_methods {
                    someip_comms
                        .rpc_oneway(
                            &someip_method.someip_service,
                            someip_method.someip_method_id,
                            someip_bytes.clone(),
                        )
                        .await?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::notifier::MockIAvailabilitySubscriber;
    use crate::agent::protocol_mapping::ServiceFqinInternal;
    use crate::agent::protocol_mapping::SomeIpEventMappingType;
    use crate::comms::{MockISdvCommsContext, MockISomeIpStackContext};
    use crate::translator::MockIBrokerTranslator;
    use google_sdv_someip::aidl::google::sdv::someip::{
        SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
    };
    use mockall::predicate::*;
    use someip_config::broker_params::SdMinorVersionMode;

    fn create_test_someip_service(
        service_id: u16,
        instance_id: u16,
        major: u8,
        minor: u32,
    ) -> SomeIpService {
        SomeIpService {
            service_id,
            instance_id,
            version: SomeIpServiceVersion { major: major as i8, minor: minor as i32 },
        }
    }

    fn create_test_sdv_service_unit_info(
        unit_type: &UnitType,
        unit_name: &str,
    ) -> SdvServiceUnitInfo {
        SdvServiceUnitInfo {
            service_unit_id: SdvServiceUnitId {
                service_fqin: ServiceFqinInternal {
                    sdv_vm_name: Some("vm".to_string()),
                    sdv_package_name: unit_type.get_sdv_package_name().to_string(),
                    service_bundle_name: unit_type.get_service_bundle_name().to_string(),
                    service_instance_name: Some("instance".to_string()),
                },
                service_unit_name: unit_name.to_string(),
            },
            unit_type: unit_type.clone(),
        }
    }

    #[test]
    fn test_add_topic_mapping() {
        // GIVEN
        let mut handler = PubSubHandler::new(
            Arc::new(MockISdvCommsContext::new()),
            Arc::new(MockIBrokerTranslator::new()),
            Duration::from_millis(100),
        );
        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.test")
            .set_service_bundle_name("Broker")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unit type must be valid");
        let service_unit_info = create_test_sdv_service_unit_info(&unit_type, "unit");
        let someip_service = create_test_someip_service(1, 1, 1, 0);
        let topic_mapping_info =
            SdvTopicMappingInfo::SimpleMapping { someip_event_id: 10, someip_eventgroup_id: 20 };

        // WHEN
        handler.add_topic_mapping(
            service_unit_info.clone(),
            someip_service,
            topic_mapping_info.clone(),
        );

        // THEN
        assert_eq!(handler.sdv_service_mappings.len(), 1);
        let stored_info =
            handler.sdv_service_mappings.get(&service_unit_info.service_unit_id).unwrap();
        assert_eq!(stored_info.someip_service, someip_service);
        assert_eq!(stored_info.sdv_topic_mapping_info, topic_mapping_info);
    }

    #[test]
    fn test_add_multpub_topic_mapping() {
        // GIVEN
        let mut handler = PubSubHandler::new(
            Arc::new(MockISdvCommsContext::new()),
            Arc::new(MockIBrokerTranslator::new()),
            Duration::from_millis(100),
        );
        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.test")
            .set_service_bundle_name("Broker")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unit type must be valid");
        let someip_service = create_test_someip_service(1, 1, 1, 0);
        let eventgroup_id = 20;
        let event_id = 10;

        // WHEN
        handler.add_multpub_topic_mapping(
            unit_type.clone(),
            someip_service,
            eventgroup_id,
            event_id,
        );

        // THEN
        assert_eq!(handler.sdv_multipub_mappings.len(), 1);
        let stored_info = handler.sdv_multipub_mappings.get(&unit_type).unwrap();
        assert_eq!(stored_info.someip_service, someip_service);
        assert_eq!(stored_info.someip_eventgroup_id, eventgroup_id);
        assert_eq!(stored_info.someip_event_id, event_id);
    }

    #[test]
    fn test_add_simple_event_mapping() {
        // GIVEN
        let mut handler = PubSubHandler::new(
            Arc::new(MockISdvCommsContext::new()),
            Arc::new(MockIBrokerTranslator::new()),
            Duration::from_millis(100),
        );
        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.test")
            .set_service_bundle_name("Broker")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unit type must be valid");
        let someip_service_instance = create_test_someip_service(1, 1, 1, 0);
        let someip_service_instance_info = SomeIpServiceInstanceInfo {
            someip_service_instance,
            sd_minor_version_mode: SdMinorVersionMode::ExactOrAnyMinorVersion,
        };
        let broker_service_unit = Box::new(BrokerServiceUnit {
            service_unit_name: "unit".to_string(),
            unit_type: unit_type.clone(),
        });
        let event_mapping_info = vec![SomeIpEventMappingInfo {
            event_id: 10,
            eventgroup_id: 20,
            event_mapping_type: SomeIpEventMappingType::SIMPLE(broker_service_unit.clone()),
        }];

        // WHEN
        handler.add_event_mapping(someip_service_instance_info, event_mapping_info.clone());

        // THEN
        let stored_info = handler.someip_service_mappings.get(&someip_service_instance).unwrap();
        assert_eq!(stored_info.len(), 1);
        assert_eq!(stored_info[0].event_id, event_mapping_info[0].event_id);
        assert_eq!(stored_info[0].eventgroup_id, event_mapping_info[0].eventgroup_id);

        let PubSubEventMappingType::SIMPLE(ref stored_service_unit) =
            stored_info[0].event_mapping_type
        else {
            panic!("Expected SIMPLE event mapping type")
        };
        assert_eq!(**stored_service_unit, *broker_service_unit)
    }

    #[test]
    fn test_add_group_event_mapping() {
        // GIVEN
        let mut handler = PubSubHandler::new(
            Arc::new(MockISdvCommsContext::new()),
            Arc::new(MockIBrokerTranslator::new()),
            Duration::from_millis(100),
        );
        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.test")
            .set_service_bundle_name("Broker")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unit type must be valid");
        let someip_service_instance = create_test_someip_service(1, 1, 1, 0);
        let someip_service_instance_info = SomeIpServiceInstanceInfo {
            someip_service_instance,
            sd_minor_version_mode: SdMinorVersionMode::ExactOrAnyMinorVersion,
        };
        let broker_service_units: Vec<BrokerServiceUnit> = (0..3)
            .map(|i| {
                let service_unit_name = format!("unit{i}");
                BrokerServiceUnit { service_unit_name, unit_type: unit_type.clone() }
            })
            .collect();
        let event_mapping_info = vec![SomeIpEventMappingInfo {
            event_id: 10,
            eventgroup_id: 20,
            event_mapping_type: SomeIpEventMappingType::GROUP(
                None,
                broker_service_units
                    .clone()
                    .into_iter()
                    .map(|unit| (unit, HashMap::new()))
                    .collect(),
            ),
        }];

        // WHEN
        handler.add_event_mapping(someip_service_instance_info, event_mapping_info.clone());

        // THEN
        let stored_info = handler.someip_service_mappings.get(&someip_service_instance).unwrap();
        assert_eq!(stored_info.len(), 1);
        assert_eq!(stored_info[0].event_id, event_mapping_info[0].event_id);
        assert_eq!(stored_info[0].eventgroup_id, event_mapping_info[0].eventgroup_id);

        let PubSubEventMappingType::GROUP(ref stored_service_units) =
            stored_info[0].event_mapping_type
        else {
            panic!("Expected GROUP event mapping type")
        };
        assert_eq!(*stored_service_units, broker_service_units)
    }

    #[tokio::test]
    async fn start_handles_someip_available_simple_event() {
        // GIVEN
        let mut sdv_comms = MockISdvCommsContext::new();
        let mut translator = MockIBrokerTranslator::new();
        let someip_comms = MockISomeIpStackContext::new();
        let mut availability_subscriber = MockIAvailabilitySubscriber::new();

        let someip_service = create_test_someip_service(1, 1, 1, 0);
        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.test")
            .set_service_bundle_name("Broker")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unit type must be valid");
        let service_unit_name = "test-unit";
        let broker_service_unit = BrokerServiceUnit {
            service_unit_name: service_unit_name.to_string(),
            unit_type: unit_type.clone(),
        };
        let event_mapping_info = vec![SomeIpEventMappingInfo {
            event_id: 10,
            eventgroup_id: 20,
            event_mapping_type: SomeIpEventMappingType::SIMPLE(Box::new(
                broker_service_unit.clone(),
            )),
        }];

        let broker_fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.android.sdv.someip")
            .service_bundle_name("SomeIpBroker")
            .service_instance_name("default")
            .build()
            .expect("Broker FQIN must be valid");

        sdv_comms.expect_get_fqin().return_const(broker_fqin.clone());

        translator
            .expect_get_publication_size_metadata()
            .with(eq(broker_fqin.clone()), eq(unit_type.clone()))
            .times(1)
            .returning(|_, _| Ok((128, 1)));

        sdv_comms
            .expect_create_publisher()
            .withf(move |args| {
                args.get_unit_name() == service_unit_name && args.get_unit_type() == unit_type
            })
            .times(1)
            .returning(|_| Err(anyhow::Error::msg("Cannot create a publisher in tests")));

        /* Ideally we would like to assert the below, but since we can't mock DT publishers, we skip it
        someip_comms
            .expect_subscribe()
            .with(eq(someip_service), eq(event_mapping_info[0].eventgroup_id), eq(event_mapping_info[0].event_id))
            .times(1)
            .returning(|_, _, _| Ok(()));

        someip_comms
            .expect_subscribe_eventgroup()
            .with(eq(someip_service), eq(event_mapping_info[0].eventgroup_id), eq(event_mapping_info[0].event_id))
            .times(1)
            .returning(|_, _, _| Ok(()));
        */
        let availability_message = crate::agent::AvailabilityMessage {
            service: ServiceType::SOMEIP(someip_service),
            available: true,
        };

        availability_subscriber
            .expect_receive()
            .times(1)
            .return_once(move || Some(availability_message));
        availability_subscriber.expect_receive().times(1).return_once(|| None);

        // WHEN
        let mut handler = PubSubHandler::new(
            Arc::new(sdv_comms),
            Arc::new(translator),
            Duration::from_millis(100),
        );
        let someip_service_instance_info = SomeIpServiceInstanceInfo {
            someip_service_instance: someip_service,
            sd_minor_version_mode: SdMinorVersionMode::ExactOrAnyMinorVersion,
        };
        handler.add_event_mapping(someip_service_instance_info, event_mapping_info.clone());

        // THEN
        let handler_arc = Arc::new(handler);
        let timeout = Duration::from_secs(1);
        let task = tokio::spawn(tokio::time::timeout(timeout, async move {
            let result =
                handler_arc.start(Box::new(availability_subscriber), Arc::new(someip_comms)).await;
            assert!(result.is_err());
        }));
        if let Err(e) = task.await {
            log::error!("Error while awaiting task {e:?}");
        }
    }

    #[tokio::test]
    async fn start_handles_someip_unavailable_simple_event() {
        // GIVEN
        let sdv_comms = MockISdvCommsContext::new(); // no calls expected
        let translator = MockIBrokerTranslator::new(); // no calls expected
        let mut someip_comms = MockISomeIpStackContext::new();
        let mut availability_subscriber = MockIAvailabilitySubscriber::new();

        let someip_service = create_test_someip_service(1, 1, 1, 0);
        let unit_type = UnitType::builder()
            .set_sdv_package_name("com.sdv.test")
            .set_service_bundle_name("Broker")
            .set_unit_type_name("TestType")
            .build()
            .expect("Unit type must be valid");
        let service_unit_name = "test-unit";
        let broker_service_unit = BrokerServiceUnit {
            service_unit_name: service_unit_name.to_string(),
            unit_type: unit_type.clone(),
        };
        let event_mapping_info = vec![SomeIpEventMappingInfo {
            event_id: 10,
            eventgroup_id: 20,
            event_mapping_type: SomeIpEventMappingType::SIMPLE(Box::new(broker_service_unit)),
        }];

        someip_comms
            .expect_unsubscribe_eventgroup()
            .with(
                eq(someip_service),
                eq(event_mapping_info[0].eventgroup_id),
                eq(vec![event_mapping_info[0].event_id]),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));

        someip_comms
            .expect_unsubscribe()
            .with(
                eq(someip_service),
                eq(event_mapping_info[0].eventgroup_id),
                eq(event_mapping_info[0].event_id),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));

        let availability_message = crate::agent::AvailabilityMessage {
            service: ServiceType::SOMEIP(someip_service),
            available: false,
        };

        availability_subscriber
            .expect_receive()
            .times(1)
            .return_once(move || Some(availability_message));
        availability_subscriber.expect_receive().times(1).return_once(|| None);

        // WHEN
        let mut handler = PubSubHandler::new(
            Arc::new(sdv_comms),
            Arc::new(translator),
            Duration::from_millis(100),
        );
        let someip_service_instance_info = SomeIpServiceInstanceInfo {
            someip_service_instance: someip_service,
            sd_minor_version_mode: SdMinorVersionMode::ExactOrAnyMinorVersion,
        };
        handler.add_event_mapping(someip_service_instance_info, event_mapping_info.clone());

        // THEN
        let handler_arc = Arc::new(handler);
        let timeout = Duration::from_secs(1);
        let task = tokio::spawn(tokio::time::timeout(timeout, async move {
            let result =
                handler_arc.start(Box::new(availability_subscriber), Arc::new(someip_comms)).await;
            assert!(result.is_err());
        }));
        if let Err(e) = task.await {
            log::error!("Error while awaiting task {e:?}");
        }
    }
}
