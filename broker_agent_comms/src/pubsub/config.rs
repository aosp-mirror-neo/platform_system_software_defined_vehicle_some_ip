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

use sdv_comms::sd::ServiceUnitDefinition;
use tokio::sync::Mutex;

use sdv_comms::dt::Publisher;
use sdv_comms::dt::Subscriber;
use sdv_comms::id::ServiceFqin;
use sdv_comms::sd::UnitType;
use sdv_status_rs::SdvResult;

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;

use crate::agent::protocol_mapping::BrokerServiceUnit;
use crate::agent::protocol_mapping::SdvTopicMappingInfo;
use crate::agent::protocol_mapping::SomeIpEvent;
use crate::agent::protocol_mapping::SomeIpEventMappingInfo;
use crate::agent::protocol_mapping::SomeIpEventMappingType;
use crate::agent::protocol_mapping::SomeIpMethod;

#[derive(Debug)]
pub(crate) struct PubSubTopicInfo {
    pub someip_service: SomeIpService,
    pub sdv_topic_mapping_info: SdvTopicMappingInfo,
}

#[derive(Debug)]
pub(crate) enum IdentifierMode {
    ServiceUnitInfo,
    Soa,
}

impl PubSubTopicInfo {
    pub(crate) fn new(
        someip_service: SomeIpService,
        sdv_topic_mapping_info: SdvTopicMappingInfo,
    ) -> Self {
        PubSubTopicInfo { someip_service, sdv_topic_mapping_info }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SdvMultipubTopicInfo {
    pub someip_service: SomeIpService,
    pub someip_eventgroup_id: u16,
    pub someip_event_id: u16,
}

impl SdvMultipubTopicInfo {
    pub(crate) fn new(
        someip_service: SomeIpService,
        someip_eventgroup_id: u16,
        someip_event_id: u16,
    ) -> Self {
        SdvMultipubTopicInfo { someip_service, someip_eventgroup_id, someip_event_id }
    }
}

pub(crate) enum BrokerPublisher {
    SIMPLE(Box<SimplePublisherInfo>),
    GROUP(Vec<SimplePublisherInfo>),
}

pub(crate) struct SimplePublisherInfo {
    pub publisher: Mutex<Publisher>,
    pub unit_type: UnitType,
}

impl SimplePublisherInfo {
    pub(crate) async fn publish(&self, event_payload: &[u8]) -> SdvResult<u32> {
        self.publisher.lock().await.write_messages(event_payload)
    }

    pub(crate) fn get_unit_type(&self) -> &UnitType {
        &self.unit_type
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct PubSubEventMappingInfo {
    pub event_id: u16,
    pub eventgroup_id: u16,
    pub event_mapping_type: PubSubEventMappingType,
}

impl From<SomeIpEventMappingInfo> for PubSubEventMappingInfo {
    fn from(value: SomeIpEventMappingInfo) -> Self {
        Self {
            event_id: value.event_id,
            eventgroup_id: value.eventgroup_id,
            event_mapping_type: value.event_mapping_type.into(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct TopicIdentifier {
    unit_type: UnitType,
    topic: String,
}

impl TopicIdentifier {
    pub(crate) fn new(unit_type: UnitType, topic: String) -> Self {
        Self { unit_type, topic }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum PubSubEventMappingType {
    SIMPLE(Box<BrokerServiceUnit>),
    GROUP(Vec<BrokerServiceUnit>),
}

impl From<SomeIpEventMappingType> for PubSubEventMappingType {
    fn from(value: SomeIpEventMappingType) -> Self {
        match value {
            SomeIpEventMappingType::SIMPLE(simple_mapping_type) => {
                PubSubEventMappingType::SIMPLE(simple_mapping_type)
            }
            SomeIpEventMappingType::GROUP(_, group_infos) => {
                // The type definition is unused, since the needed metadata for translation has already been created and cached for use by the translator
                PubSubEventMappingType::GROUP(group_infos.into_iter().map(|info| info.0).collect())
            }
        }
    }
}

pub(crate) struct BrokerMultipubSubscriberInfo {
    // Specifies the event that the broker should publish to when a message is received from this publication
    pub someip_event: SomeIpEvent,
    // Service FQIN of the first discovered publisher. The VSIDL config belonging to this FQIN will be used
    // when translating messages
    pub first_publisher_fqin: ServiceFqin,
    // Unit Type corresponding to this publication
    pub unit_type: UnitType,
    // Handle to the subscriber created by the broker for this publication
    pub subscriber: Mutex<Option<Subscriber>>,
    // Size of the message for this publication
    pub message_size: usize,
    // Current count of publishers of this publication
    pub publisher_count: u32,
}

pub(crate) struct BrokerSoaSubscriberInfo {
    // Specifies the action that the broker should take when a message is received from this topic
    pub subscriber_type: BrokerSubscriberType,
    // Service FQIN of the first discovered publisher. The VSIDL config belonging to this FQIN will be used
    // when translating messages
    pub first_publisher_fqin: ServiceFqin,
    // Topic name corresponding to this publication
    pub topic_name: String,
    // Unit type corresponding to this publication
    pub unit_type: UnitType,
    // Handle to the subscriber created by the broker for this publication
    pub subscriber: Mutex<Option<Subscriber>>,
    // Size of the message for this publication
    pub message_size: usize,
    // Current count of publishers of this publication
    pub publisher_count: u32,
}

pub(crate) struct BrokerSubscriberInfo {
    // Specifies the action that the broker should take when a message is received from this publication
    pub subscriber_type: BrokerSubscriberType,
    // ServiceUnitDefinition corresponding to this publication
    pub service_unit_definition: Box<ServiceUnitDefinition>,
    // Handle to the subscriber created by the broker for this publication
    pub subscriber: Mutex<Option<Subscriber>>,
    // Size of the message for this publication
    pub message_size: usize,
}

#[derive(Clone)]
pub(crate) enum BrokerSubscriberType {
    SIMPLE(SomeIpEvent),
    FF(Vec<SomeIpMethod>),
}

/// Identifier for SOME/IP events that the broker is subscribed to.
/// Information about the minor version and eventgroup is omitted from
/// the usual definition of a SOME/IP event
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct SomeIpEventSubscriberInfo {
    /// Service ID of the event offerer
    pub service_id: u16,
    /// Instance ID of the event offerer
    pub instance_id: u16,
    /// Major version of the event offerer
    pub version_major: u8,
    /// Event ID
    pub event_id: u16,
}

impl SomeIpEventSubscriberInfo {
    pub(crate) fn new(someip_service: SomeIpService, event_id: u16) -> SomeIpEventSubscriberInfo {
        SomeIpEventSubscriberInfo {
            service_id: someip_service.service_id,
            instance_id: someip_service.instance_id,
            version_major: someip_service.version.major as u8,
            event_id,
        }
    }
}

pub(crate) enum PollableSubscriber<'a> {
    Single(&'a BrokerSubscriberInfo),
    MultiPub(&'a BrokerMultipubSubscriberInfo),
    Soa(&'a BrokerSoaSubscriberInfo),
}

impl PollableSubscriber<'_> {
    pub(crate) fn message_size(&self) -> usize {
        match self {
            PollableSubscriber::Single(info) => info.message_size,
            PollableSubscriber::MultiPub(info) => info.message_size,
            PollableSubscriber::Soa(info) => info.message_size,
        }
    }

    pub(crate) fn get_unit_type(&self) -> UnitType {
        match self {
            PollableSubscriber::Single(info) => info.service_unit_definition.get_unit_type(),
            PollableSubscriber::MultiPub(info) => info.unit_type.clone(),
            PollableSubscriber::Soa(info) => info.unit_type.clone(),
        }
    }

    pub(crate) fn get_fqin(&self) -> ServiceFqin {
        match self {
            PollableSubscriber::Single(info) => {
                info.service_unit_definition.get_service_bundle_identity().get_fqin().clone()
            }
            PollableSubscriber::MultiPub(info) => info.first_publisher_fqin.clone(),
            PollableSubscriber::Soa(info) => info.first_publisher_fqin.clone(),
        }
    }

    pub(crate) fn get_subscriber_type(&self) -> BrokerSubscriberType {
        match self {
            PollableSubscriber::Single(info) => info.subscriber_type.clone(),
            PollableSubscriber::MultiPub(info) => BrokerSubscriberType::SIMPLE(info.someip_event),
            PollableSubscriber::Soa(info) => info.subscriber_type.clone(),
        }
    }
}
