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
    use googletest::matcher::Matcher;
    use googletest::matchers::near;

    use log::{debug, error};

    use tokio::time::timeout;
    use tokio::time::Duration;

    use sdv::mw::SdvComms;
    use sdv::mw::SubscribeOptions;
    use sdv::status::SdvResult;
    use sdv_comms::id::ServiceFqin;
    use sdv_comms::ContextRef;

    use sdv_someip_integration_test::run_vsomeip_server;

    use someip_messages_catalog::side_mirror::SideMirrorPosition;

    use sdv_mw_rs_com_sdv_google_sample_someip_side_mirror_test_client::{
        subscriber, SideMirrorTestClient, UnitType,
    };

    const TEST_TIMEOUT_IN_MS: u32 = 3000;

    const PUBLISHER_WAIT_TIME: u64 = 3000;
    const PUBLISHER_POLL_INTERVAL: u64 = 200;

    const VSOMEIP_SERVER_BINARY: &str = "/vendor/bin/sdv_vsomeip_side_mirror_tester";
    const VSOMEIP_CONFIG_FILE: &str = "/vendor/etc/vsomeip/side_mirror_tester.json";

    #[tokio::test]
    async fn test_someip_broker_multi_instance() {
        sdv_log::init_logger("test_someip_broker_multi_instance").unwrap();

        let mut server = run_vsomeip_server(VSOMEIP_SERVER_BINARY, VSOMEIP_CONFIG_FILE);

        run_sdv_client().await;

        let _ = server.kill();
        let _ = server.wait();
    }

    fn almost_equal(actual: f32, expected: f32) -> bool {
        const EPSILON: f32 = 0.0001f32;
        near(expected, EPSILON).matches(actual).is_match()
    }

    async fn run_sdv_client() {
        let sdv_comms = SdvComms { context: create_context() };
        let mut test_client =
            SideMirrorTestClient::new(Arc::new(sdv_comms)).await.expect("Test Client should start");
        debug!("Client started");

        let mut left_side_mirror_observer =
            wait_for_observer(&mut test_client, subscriber::Variant::<SideMirrorPosition>::LEFT)
                .await
                .expect("Unable to create left side mirror observer");

        let mut right_side_mirror_observer =
            wait_for_observer(&mut test_client, subscriber::Variant::<SideMirrorPosition>::RIGHT)
                .await
                .expect("Unable to create right side mirror observer");

        let left_side_mirror_handle = tokio::spawn(async move {
            match left_side_mirror_observer.next().await {
                Some(Ok(messages)) => {
                    let Some(message) = messages.into_iter().next() else {
                        panic!("No messages received for left side mirror position");
                    };
                    debug!(
                        "Received side mirror position with vertical angle {0}, horizontal angle {1}",
                        message.vertical_angle, message.horizontal_angle
                    );
                    if almost_equal(message.vertical_angle, 10f32)
                        && almost_equal(message.horizontal_angle, 10f32)
                    {
                        true
                    } else {
                        error!("Received wrong message data for SideMirrorPosition::Left, vertical angle = {0}, horizontal angle {1}", message.vertical_angle, message.horizontal_angle);
                        false
                    }
                }
                Some(Err(_)) => {
                    // error reading message
                    panic!("Error while deserializing left side mirror message");
                }
                _ => {
                    panic!("Got none on left side mirror observer");
                }
            }
        });

        let right_side_mirror_handle = tokio::spawn(async move {
            match right_side_mirror_observer.next().await {
                Some(Ok(messages)) => {
                    let Some(message) = messages.into_iter().next() else {
                        panic!("No messages received for right side mirror position");
                    };
                    debug!(
                        "Received side mirror position with vertical angle {0}, horizontal angle {1}",
                        message.vertical_angle, message.horizontal_angle
                    );
                    if almost_equal(message.vertical_angle, 20f32)
                        && almost_equal(message.horizontal_angle, 20f32)
                    {
                        true
                    } else {
                        error!("Received wrong message data for SideMirrorPosition::Right, vertical angle = {0}, horizontal angle {1}", message.vertical_angle, message.horizontal_angle);
                        false
                    }
                }
                Some(Err(_)) => {
                    // error reading message
                    panic!("Error while deserializing right side mirror message");
                }
                _ => {
                    panic!("Got none on right side mirror observer");
                }
            }
        });

        let left_side_mirror_match =
            timeout(Duration::from_millis(TEST_TIMEOUT_IN_MS.into()), left_side_mirror_handle)
                .await
                .expect("Timed out waiting for left exit condition")
                .expect("Error waiting for left exit condition");

        let right_side_mirror_match =
            timeout(Duration::from_millis(TEST_TIMEOUT_IN_MS.into()), right_side_mirror_handle)
                .await
                .expect("Timed out waiting for right exit condition")
                .expect("Error waiting for right exit condition");

        assert!(left_side_mirror_match && right_side_mirror_match,);
    }

    async fn wait_for_observer<T: protobuf::Message + subscriber::Metadata + UnitType>(
        test_client: &mut SideMirrorTestClient,
        variant: &subscriber::Variant<T>,
    ) -> SdvResult<std::pin::Pin<Box<dyn futures::stream::Stream<Item = SdvResult<Vec<T>>> + Send>>>
    {
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
            .sdv_package_name("com.sdv.google.sample.someip")
            .service_bundle_name("SideMirrorTestClient")
            .service_instance_name("instance1")
            .build()
            .unwrap();

        ContextRef::create(fqin)
    }
}
