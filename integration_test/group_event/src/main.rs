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
//!  SOME/IP integration test.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::StreamExt;

    use log::{debug, error};

    use tokio::time::timeout;
    use tokio::time::Duration;

    use sdv_comms::id::ServiceFqin;
    use sdv_comms::ContextRef;
    use sdv_mw_rs_comms::SubscribeOptions;
    use sdv_mw_rs_sdv_comms::SdvComms;

    use sdv_someip_integration_test::run_vsomeip_server;
    use someip_messages_catalog::first_part::FirstPart;
    use someip_messages_catalog::second_part::SecondPart;

    use sdv_mw_rs_com_android_sdv_sample_someip_someip_group_event_consumer::{
        subscriber, SomeipGroupEventConsumer, UnitType,
    };

    // Additional 3s of time to account for polling interval and infrastructure issues.
    const TIMEOUT: u32 = 4000;
    const PUBLISHER_WAIT_TIME: u64 = 3000;
    const PUBLISHER_POLL_INTERVAL: u64 = 200;
    const VSOMEIP_SERVER_BINARY: &str = "/vendor/bin/sdv_vsomeip_group_tester";
    const VSOMEIP_CONFIG_FILE: &str = "/vendor/etc/vsomeip/group_tester.json";

    #[tokio::test]
    async fn test_someip_broker_group_event() {
        sdv_log::init_logger("test_someip_broker_group_event").unwrap();

        let mut server = run_vsomeip_server(VSOMEIP_SERVER_BINARY, VSOMEIP_CONFIG_FILE);

        run_sdv_client().await;

        let _ = server.kill();
        let _ = server.wait();
    }

    async fn run_sdv_client() {
        let sdv_comms = SdvComms { context: create_context() };
        let mut test_client = SomeipGroupEventConsumer::new(Arc::new(sdv_comms))
            .await
            .expect("Test Client should start");
        debug!("Client started");

        let mut first_part_observer =
            wait_for_observer(&mut test_client, subscriber::Variant::<FirstPart>::UNIQUE)
                .await
                .expect("Unable to create first part observer");

        let mut second_part_observer =
            wait_for_observer(&mut test_client, subscriber::Variant::<SecondPart>::UNIQUE)
                .await
                .expect("Unable to create second part observer");

        let first_part_handle = tokio::spawn(async move {
            match first_part_observer.next().await {
                Some(Ok(messages)) => {
                    let Some(message) = messages.into_iter().next() else {
                        panic!("No messages received for second part unit type");
                    };
                    if !message.sample_boolean {
                        error!("Unexpected value received: {}", message.sample_boolean);
                        false
                    } else {
                        true
                    }
                }
                Some(Err(_)) => {
                    panic!("Error while deserializing first part message");
                }
                _ => {
                    panic!("Got none on first part observer");
                }
            }
        });

        let second_part_handle = tokio::spawn(async move {
            match second_part_observer.next().await {
                Some(Ok(messages)) => {
                    let Some(message) = messages.into_iter().next() else {
                        panic!("No messages received for second part unit type");
                    };
                    if message.sample_int != 42 {
                        error!("Unexpected value received: {}", message.sample_int);
                        false
                    } else {
                        true
                    }
                }
                Some(Err(_)) => {
                    panic!("Error while deserializing second part message");
                }
                _ => {
                    panic!("Got none on second part observer");
                }
            }
        });

        let first_part_match = timeout(Duration::from_millis(TIMEOUT.into()), first_part_handle)
            .await
            .expect("Timed out waiting for first part exit condition")
            .expect("Error waiting for first part exit condition");

        let second_part_match = timeout(Duration::from_millis(TIMEOUT.into()), second_part_handle)
            .await
            .expect("Timed out waiting for second part exit condition")
            .expect("Error waiting for second part exit condition");

        assert!(first_part_match && second_part_match);
    }

    async fn wait_for_observer<T: protobuf::Message + subscriber::Metadata + UnitType>(
        test_client: &mut SomeipGroupEventConsumer,
        variant: &subscriber::Variant<T>,
    ) -> sdv_status_rs::SdvResult<
        std::pin::Pin<
            Box<dyn futures::stream::Stream<Item = sdv_status_rs::SdvResult<Vec<T>>> + Send>,
        >,
    > {
        let mut counter = 0;
        let counter_end = PUBLISHER_WAIT_TIME / PUBLISHER_POLL_INTERVAL;
        loop {
            match test_client
                .create_observer(
                    variant,
                    &SubscribeOptions::builder()
                        .read_on_subscribe(true)
                        .min_interval(std::time::Duration::from_secs(1))
                        .build()?,
                )
                .await
            {
                Ok(observer) => {
                    return Ok(observer);
                }
                Err(e) => {
                    tokio::time::sleep(Duration::from_millis(PUBLISHER_POLL_INTERVAL)).await;
                    counter += 1;
                    if counter == counter_end {
                        return Err(e);
                    }
                }
            }
        }
    }

    fn create_context() -> ContextRef {
        let fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.android.sdv.sample.someip")
            .service_bundle_name("SomeipGroupEventConsumer")
            .service_instance_name("instance1")
            .build()
            .unwrap();

        ContextRef::create(fqin)
    }
}
