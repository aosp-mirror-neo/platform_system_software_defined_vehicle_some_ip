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
    use std::process::Child;
    use std::sync::Arc;

    use async_trait::async_trait;
    use log::error;

    use tokio::time::sleep;
    use tokio::time::Duration;

    use sdv_comms::id::ServiceFqin;
    use sdv_comms::ContextRef;

    use com_sdv_google_sample_someip_test_hello_rpc::Interface as TestHello;
    use sdv_mw_rs_com_sdv_google_sample_someip_sdv_integration_test_server::SdvIntegrationTestServer;
    use sdv_mw_rs_sdv_comms::SdvComms;
    use someip_messages_catalog::test_hello_rpc::TestHelloRequest;
    use someip_messages_catalog::test_hello_rpc::TestHelloResponse;

    use sdv_someip_integration_test::run_vsomeip_server;

    struct TestHelloImpl {}

    impl TestHelloImpl {
        fn new() -> Self {
            TestHelloImpl {}
        }
    }

    #[async_trait]
    impl TestHello for TestHelloImpl {
        async fn SayHello(
            &self,
            _caller_id: sdv_comms::id::ServiceFqin,
            request: &TestHelloRequest,
        ) -> sdv_status_rs::SdvResult<TestHelloResponse> {
            log::info!("Received request to say hello! Request: {}", &request.request);
            let mut response = "SDV Hello ".to_owned();
            response.push_str(&request.request);
            Ok(TestHelloResponse { response, ..Default::default() })
        }
    }

    const VSOMEIP_CLIENT_BINARY: &str = "/vendor/bin/sdv_vsomeip_sdv_hello_tester";
    const VSOMEIP_CONFIG_FILE: &str = "/vendor/etc/vsomeip/sdv_hello_tester.json";
    const TEST_START_WAIT_TIME_IN_SECS: u64 = 1;
    const TIMEOUT_IN_SECS: u64 = 5;

    #[tokio::test]
    async fn test_someip_broker_rpc_sdv_server() {
        sdv_log::init_logger("test_someip_broker_rpc_sdv").unwrap();

        let sdv_comms = SdvComms { context: create_context() };
        let _service =
            SdvIntegrationTestServer::new(Arc::new(sdv_comms), (Arc::new(TestHelloImpl::new()),))
                .await
                .expect("Unable to create integration test server");

        sleep(Duration::from_secs(TEST_START_WAIT_TIME_IN_SECS)).await;
        let mut client_process = run_vsomeip_server(VSOMEIP_CLIENT_BINARY, VSOMEIP_CONFIG_FILE);

        let result = wait_for_exit_with_timeout(&mut client_process).await;

        assert!(result, "Test failed to receive RPC response within timeout");
    }

    async fn wait_for_exit_with_timeout(process: &mut Child) -> bool {
        const POLL_INTERVAL: Duration = Duration::from_millis(100); // Adjust as needed
        let counter_end = TIMEOUT_IN_SECS * 10;
        let mut counter = 0;
        loop {
            match process.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) => {
                    counter += 1;

                    if counter >= counter_end {
                        error!("VSomeIP client timed out");
                        return false; // Timeout occurred
                    }
                    log::info!("Sleeping");
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                Err(e) => panic!("Error while waiting for client binary: {e}"),
            }
        }
    }

    fn create_context() -> ContextRef {
        let fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.sdv.google.sample.someip")
            .service_bundle_name("SdvIntegrationTestServer")
            .service_instance_name("instance1")
            .build()
            .unwrap();

        ContextRef::create(fqin)
    }
}
