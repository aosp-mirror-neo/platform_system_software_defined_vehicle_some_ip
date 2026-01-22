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
use binder_tokio::Tokio;
use mockall::automock;

use google_sdv_someip::{
    aidl::google::sdv::someip::{
        AvailabilityStatus::AvailabilityStatus,
        IEventNotificationCallback::BnEventNotificationCallback,
        IRpcRequestCallback::BnRpcRequestCallback,
        ISomeIpServiceAvailabilityCallback::BnSomeIpServiceAvailabilityCallback,
        ISomeIpStack::ISomeIpStackAsync, SomeIpService::SomeIpService,
    },
    binder,
    binder::Strong,
};

use crate::pubsub::EventNotificationCallback;
use crate::rpc::RpcRequestCallback;
use crate::service_discovery::AvailabilityCallback;

const SOMEIP_STACK_AGENT: &str = "google.sdv.someip.ISomeIpStack/default";

#[automock]
#[async_trait]
pub(crate) trait ISomeIpStackContextFactory {
    async fn create(
        &self,
        service_availability_callback: AvailabilityCallback,
        event_callback: EventNotificationCallback,
        rpc_callback: RpcRequestCallback,
    ) -> anyhow::Result<Box<dyn ISomeIpStackContext>>;
}

pub(crate) struct SomeIpStackContextFactoryImpl {}

#[async_trait]
impl ISomeIpStackContextFactory for SomeIpStackContextFactoryImpl {
    async fn create(
        &self,
        service_availability_callback: AvailabilityCallback,
        event_callback: EventNotificationCallback,
        rpc_callback: RpcRequestCallback,
    ) -> anyhow::Result<Box<dyn ISomeIpStackContext>> {
        let someip_stack =
            binder_tokio::get_interface::<dyn ISomeIpStackAsync<Tokio>>(SOMEIP_STACK_AGENT)
                .await
                .map_err(|_| anyhow::Error::msg("Unable to get SOME/IP stack."))?;

        let rpc_request_callback = BnRpcRequestCallback::new_async_binder(
            rpc_callback,
            binder_tokio::TokioRuntime(tokio::runtime::Handle::current()),
            binder::BinderFeatures::default(),
        );
        someip_stack
            .register_rpc_request_callback(&rpc_request_callback)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to register RPC request callback."))?;

        let event_notification_callback = BnEventNotificationCallback::new_async_binder(
            event_callback,
            binder_tokio::TokioRuntime(tokio::runtime::Handle::current()),
            binder::BinderFeatures::default(),
        );
        someip_stack
            .register_event_notification_callback(&event_notification_callback)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to register event notification callback."))?;

        let availability_callback = BnSomeIpServiceAvailabilityCallback::new_async_binder(
            service_availability_callback,
            binder_tokio::TokioRuntime(tokio::runtime::Handle::current()),
            binder::BinderFeatures::default(),
        );
        someip_stack
            .register_availability_callback(&availability_callback)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to register service availability callback."))?;

        Ok(Box::new(SomeIpStackContext::new(someip_stack)))
    }
}

#[automock]
#[async_trait]
pub(crate) trait ISomeIpStackContext: Sync + Send {
    async fn monitor_service(&self, someip_service: &SomeIpService) -> anyhow::Result<()>;

    async fn offer_service(&self, someip_service: &SomeIpService) -> anyhow::Result<()>;

    async fn stop_offer_service(&self, someip_service: &SomeIpService) -> anyhow::Result<()>;

    async fn subscribe(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()>;

    async fn unsubscribe(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()>;

    async fn offer_sdv_topic(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()>;

    async fn stop_offer_sdv_topic(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()>;

    async fn publish(
        &self,
        someip_service: &SomeIpService,
        event_id: u16,
        payload: Vec<u8>,
    ) -> anyhow::Result<()>;

    async fn rpc_oneway(
        &self,
        someip_service: &SomeIpService,
        method_id: u16,
        payload: Vec<u8>,
    ) -> anyhow::Result<()>;

    async fn rpc_transact(
        &self,
        someip_service: &SomeIpService,
        method_id: u16,
        payload: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>>;

    async fn subscribe_eventgroup(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_ids: &[u16],
    ) -> anyhow::Result<()>;

    async fn unsubscribe_eventgroup(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_ids: &[u16],
    ) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub(crate) struct SomeIpStackContext {
    someip_stack: Strong<dyn ISomeIpStackAsync<Tokio>>,
}

impl SomeIpStackContext {
    pub(crate) fn new(someip_stack: Strong<dyn ISomeIpStackAsync<Tokio>>) -> Self {
        Self { someip_stack }
    }
}

#[async_trait]
impl ISomeIpStackContext for SomeIpStackContext {
    async fn monitor_service(&self, someip_service: &SomeIpService) -> anyhow::Result<()> {
        self.someip_stack
            .monitor_service(someip_service)
            .await
            .map_err(|e| anyhow::anyhow!("Couldn't access stack, error {e:?}"))
    }

    async fn offer_service(&self, someip_service: &SomeIpService) -> anyhow::Result<()> {
        self.someip_stack
            .on_sdv_service_availability(someip_service, AvailabilityStatus::AVAILABLE)
            .await
            .map_err(|e| anyhow::anyhow!("Couldn't access stack, error {e:?}"))
    }

    async fn stop_offer_service(&self, someip_service: &SomeIpService) -> anyhow::Result<()> {
        self.someip_stack
            .on_sdv_service_availability(someip_service, AvailabilityStatus::UNAVAILABLE)
            .await
            .map_err(|e| anyhow::anyhow!("Couldn't access stack, error {e:?}"))
    }

    async fn subscribe(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()> {
        self.someip_stack
            .subscribe(someip_service, eventgroup, event_id)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to subscribe to SOME/IP event."))
    }

    async fn unsubscribe(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()> {
        self.someip_stack
            .unsubscribe(someip_service, eventgroup, event_id)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to unsubscribe to SOME/IP event."))
    }

    async fn offer_sdv_topic(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()> {
        self.someip_stack
            .on_sdv_topic_availability(
                someip_service,
                eventgroup,
                event_id,
                AvailabilityStatus::AVAILABLE,
            )
            .await
            .map_err(|_| anyhow::Error::msg("Unable to Offer SDV topic as SOME/IP event."))
    }

    async fn stop_offer_sdv_topic(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_id: u16,
    ) -> anyhow::Result<()> {
        self.someip_stack
            .on_sdv_topic_availability(
                someip_service,
                eventgroup,
                event_id,
                AvailabilityStatus::UNAVAILABLE,
            )
            .await
            .map_err(|_| anyhow::Error::msg("Unable to Stop Offer SDV topic as SOME/IP event."))
    }

    async fn publish(
        &self,
        someip_service: &SomeIpService,
        event_id: u16,
        payload: Vec<u8>,
    ) -> anyhow::Result<()> {
        self.someip_stack
            .publish(someip_service, event_id, &payload)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to publish SDV topic as SOME/IP event."))
    }

    async fn rpc_oneway(
        &self,
        someip_service: &SomeIpService,
        method_id: u16,
        payload: Vec<u8>,
    ) -> anyhow::Result<()> {
        self.someip_stack.rpc_oneway(someip_service, method_id, &payload).await.map_err(|_| {
            anyhow::Error::msg("Unable to send SDV topic as SOME/IP RPC oneway method.")
        })
    }

    async fn rpc_transact(
        &self,
        someip_service: &SomeIpService,
        method_id: u16,
        payload: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>> {
        self.someip_stack
            .rpc_transact(someip_service, method_id, &payload)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to send SDV topic as SOME/IP RPC method."))
    }

    async fn subscribe_eventgroup(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_ids: &[u16],
    ) -> anyhow::Result<()> {
        self.someip_stack
            .subscribe_eventgroup(someip_service, eventgroup, event_ids)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to subscribe to SOME/IP events."))
    }

    async fn unsubscribe_eventgroup(
        &self,
        someip_service: &SomeIpService,
        eventgroup: u16,
        event_ids: &[u16],
    ) -> anyhow::Result<()> {
        self.someip_stack
            .unsubscribe_eventgroup(someip_service, eventgroup, event_ids)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to unsubscribe to SOME/IP events."))
    }
}
