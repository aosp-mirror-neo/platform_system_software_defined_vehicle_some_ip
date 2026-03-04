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

    use tokio::sync::watch;
    use tokio::sync::watch::Sender;
    use tokio::sync::Mutex;
    use tokio::time::timeout;
    use tokio::time::Duration;

    use sdv_comms::id::ServiceFqin;
    use sdv_comms::ContextRef;
    use sdv_mw_rs_com_android_sdv_sample_someip_test_client::{
        publisher, subscriber, TestClient, UnitType,
    };
    use sdv_mw_rs_comms::SubscribeOptions;
    use sdv_mw_rs_sdv_comms::SdvComms;
    use sdv_someip_integration_test::run_vsomeip_server;
    use someip_messages_catalog::counter_load::CounterLoad;
    use someip_messages_catalog::counter_load::Overflow;

    const STARTING_COUNTER: u32 = 5;
    const TICK_INTERVAL_IN_MILLIS: u32 = 250;

    const TEST_START_WAIT_TIME_IN_MILLIS: u64 = 2000;
    const PUBLISHER_POLL_INTERVAL: u64 = 200;
    const TEST_TIMEOUT_IN_MILLIS: u64 = 3000;

    const VSOMEIP_SERVER_BINARY: &str = "/vendor/bin/sdv_vsomeip_counter_tester";
    const VSOMEIP_CONFIG_FILE: &str = "/vendor/etc/vsomeip/counter_tester.json";

    #[derive(Debug)]
    struct TestState {
        received_count: i32,
        expected_value: i32,
        success: bool,
        overflow: bool,
        callback_error: Vec<String>,
    }

    impl TestState {
        fn new() -> Self {
            // Server starts from STARTING_COUNTER but decrease it by 1 before sending
            Self {
                received_count: 0,
                expected_value: i32::try_from(STARTING_COUNTER).unwrap() - 1,
                success: false,
                overflow: false,
                callback_error: Vec::new(),
            }
        }
    }

    #[tokio::test]
    async fn test_someip_broker_pub_sub() {
        sdv_log::init_logger("test_someip_broker_pubsub").unwrap();

        let mut server = run_vsomeip_server(VSOMEIP_SERVER_BINARY, VSOMEIP_CONFIG_FILE);

        let test_state = Arc::new(Mutex::new(TestState::new()));
        run_sdv_client(test_state.clone()).await;

        let _ = server.kill();
        let _ = server.wait();

        let test_state = test_state.lock().await;
        assert!(
            test_state.success && test_state.overflow,
            "Failed to complete. State: {test_state:?}"
        );
    }

    fn notify_callback_error(errors: &mut Vec<String>, error_text: String, sender: &Sender<bool>) {
        errors.push(error_text);
        sender.send(true).expect("Unable to signal exit condition");
    }

    async fn run_sdv_client(state: Arc<Mutex<TestState>>) {
        let sdv_comms = SdvComms { context: create_context() };
        let mut test_client =
            TestClient::new(Arc::new(sdv_comms)).await.expect("Test Client should start");
        debug!("Client started");

        let mut overflow_observer =
            wait_for_observer(&mut test_client, subscriber::Variant::<Overflow>::OVERFLOW)
                .await
                .expect("Unable to create overflow observer");

        let mut counter_observer =
            wait_for_observer(&mut test_client, subscriber::Variant::<CounterLoad>::LOAD)
                .await
                .expect("Unable to create counter load observer");

        let counter_state_clone = state.clone();
        let overflow_state_clone = state.clone();

        let (overflow_exit_condition, mut overflow_exit_condition_receiver) = watch::channel(false);
        let (counter_exit_condition, mut counter_exit_condition_receiver) = watch::channel(false);

        tokio::spawn(async move {
            match overflow_observer.next().await {
                Some(Ok(_messages)) => {
                    let mut state = overflow_state_clone.lock().await;
                    state.overflow = true;
                    overflow_exit_condition
                        .send(true)
                        .expect("Unable to signal overflow condition");
                }
                Some(Err(err)) => {
                    let mut state = overflow_state_clone.lock().await;
                    // error reading message
                    notify_callback_error(
                        &mut state.callback_error,
                        format!("Error while deserializing overflow request message: {err:?}"),
                        &overflow_exit_condition,
                    );
                }
                _ => {
                    error!("Got none on overflow observer");
                }
            }
        });

        tokio::spawn(async move {
            loop {
                match counter_observer.next().await {
                    Some(Ok(messages)) => {
                        let mut state = counter_state_clone.lock().await;
                        for message in messages {
                            state.received_count += 1;

                            if i32::try_from(message.counter) != Ok(state.expected_value) {
                                notify_callback_error(
                                    &mut state.callback_error,
                                    format!("Unexpected value received: {}", message.counter),
                                    &counter_exit_condition,
                                );
                                return;
                            };

                            if message.counter == 0 {
                                state.success = true;
                                counter_exit_condition
                                    .send(true)
                                    .expect("Unable to signal overflow condition");
                            }
                            state.expected_value -= 1;
                        }
                    }
                    Some(Err(_)) => {
                        // error reading message
                        let mut state = counter_state_clone.lock().await;
                        notify_callback_error(
                            &mut state.callback_error,
                            "Error while deserializing counter_load request message".to_string(),
                            &counter_exit_condition,
                        );
                        return;
                    }
                    _ => {
                        error!("Got none on counter load observer");
                    }
                }
            }
        });

        let start_publisher = test_client
            .take_publisher(publisher::Variant::<CounterLoad>::START_COUNTER)
            .expect("Unable to take START_COUNTER publisher");

        start_publisher
            .publish(&CounterLoad {
                interval: TICK_INTERVAL_IN_MILLIS,
                counter: STARTING_COUNTER,
                ..Default::default()
            })
            .expect("Error publishing");

        timeout(
            Duration::from_millis(TEST_TIMEOUT_IN_MILLIS),
            overflow_exit_condition_receiver.wait_for(|overflow| *overflow),
        )
        .await
        .expect("Timed out waiting for overflow exit condition")
        .expect("Error waiting for overflow exit condition");

        timeout(
            Duration::from_millis(TEST_TIMEOUT_IN_MILLIS),
            counter_exit_condition_receiver.wait_for(|counter| *counter),
        )
        .await
        .expect("Timed out waiting for counter exit condition")
        .expect("Error waiting for counter condition");
    }

    // TODO(b/372453493): Avoid manual waiting for publishers
    async fn wait_for_observer<T: protobuf::Message + subscriber::Metadata + UnitType>(
        test_client: &mut TestClient,
        variant: &subscriber::Variant<T>,
    ) -> sdv_status_rs::SdvResult<
        std::pin::Pin<
            Box<dyn futures::stream::Stream<Item = sdv_status_rs::SdvResult<Vec<T>>> + Send>,
        >,
    > {
        let mut counter = 0;
        let counter_end = TEST_START_WAIT_TIME_IN_MILLIS.div_ceil(PUBLISHER_POLL_INTERVAL);
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
            .service_bundle_name("TestClient")
            .service_instance_name("instance1")
            .build()
            .unwrap();

        ContextRef::create(fqin)
    }
}
