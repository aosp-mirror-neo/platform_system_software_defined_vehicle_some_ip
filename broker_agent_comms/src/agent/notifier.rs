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
use mockall::automock;
use tokio::sync::mpsc::{Receiver as TokioChannelReceiver, Sender as TokioChannelSender};

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;
use sdv_comms::sd::ServiceUnitDefinition;

#[allow(dead_code)]
pub(crate) enum ServiceType {
    SDV(Box<ServiceUnitDefinition>),
    SOMEIP(SomeIpService),
}

#[allow(dead_code)]
pub(crate) struct AvailabilityMessage {
    pub service: ServiceType,
    pub available: bool,
}

#[automock]
#[async_trait]
pub(crate) trait IAvailabilityNotifier: Send + Sync {
    async fn notify(&self, notification: AvailabilityMessage) -> anyhow::Result<()>;
    fn clone_box(&self) -> Box<dyn IAvailabilityNotifier>;
}

#[async_trait]
impl IAvailabilityNotifier for TokioChannelSender<AvailabilityMessage> {
    async fn notify(&self, notification: AvailabilityMessage) -> anyhow::Result<()> {
        self.send(notification).await.map_err(anyhow::Error::from)
    }

    fn clone_box(&self) -> Box<dyn IAvailabilityNotifier> {
        Box::new(self.clone())
    }
}

#[automock]
#[async_trait]
pub(crate) trait IAvailabilitySubscriber: Send {
    async fn receive(&mut self) -> Option<AvailabilityMessage>;
}

#[async_trait]
impl IAvailabilitySubscriber for TokioChannelReceiver<AvailabilityMessage> {
    async fn receive(&mut self) -> Option<AvailabilityMessage> {
        self.recv().await
    }
}

#[automock]
pub(crate) trait IAvailabilityMessageFactory {
    fn create_messaging_channel(
        &self,
    ) -> anyhow::Result<(Box<dyn IAvailabilityNotifier>, Box<dyn IAvailabilitySubscriber>)>;
}

pub(crate) struct ChannelMessageFactory {
    buffer: usize,
}

impl ChannelMessageFactory {
    pub(crate) fn new(buffer: usize) -> Self {
        ChannelMessageFactory { buffer }
    }
}

#[async_trait]
impl IAvailabilityMessageFactory for ChannelMessageFactory {
    fn create_messaging_channel(
        &self,
    ) -> anyhow::Result<(Box<dyn IAvailabilityNotifier>, Box<dyn IAvailabilitySubscriber>)> {
        let (notifier, subscriber) = tokio::sync::mpsc::channel::<AvailabilityMessage>(self.buffer);
        Ok((Box::new(notifier), Box::new(subscriber)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_sdv_someip::aidl::google::sdv::someip::{
        SomeIpService::SomeIpService, SomeIpServiceVersion::SomeIpServiceVersion,
    };

    #[tokio::test]
    async fn test_availability_message_with_someip_service_type() {
        let channel_msg_factory = ChannelMessageFactory::new(10);
        let (notifier, mut subscriber) = channel_msg_factory
            .create_messaging_channel()
            .expect("Failed to create messaging channel");

        let someip_version = SomeIpServiceVersion { major: 1, minor: 2 };
        let someip_instance =
            SomeIpService { service_id: 123, instance_id: 456, version: someip_version };
        let message1 =
            AvailabilityMessage { service: ServiceType::SOMEIP(someip_instance), available: true };

        let message2 =
            AvailabilityMessage { service: ServiceType::SOMEIP(someip_instance), available: true };

        let notifier_clone = notifier.clone_box();
        notifier.notify(message1).await.expect("Failed to notify");
        notifier_clone.notify(message2).await.expect("Failed to notify");

        let received_msg1 = subscriber.receive().await.expect("Failed to receive message");
        let received_msg2 = subscriber.receive().await.expect("Failed to receive message");

        assert!(received_msg1.available);
        assert!(received_msg2.available);

        assert!(
            matches!(&received_msg1.service, ServiceType::SOMEIP(service) if *service == someip_instance)
        );
        assert!(
            matches!(&received_msg2.service, ServiceType::SOMEIP(service) if *service == someip_instance)
        );
    }
}
