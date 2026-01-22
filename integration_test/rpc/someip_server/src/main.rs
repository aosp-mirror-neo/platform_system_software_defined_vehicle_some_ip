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

    use sdv_mw_rs_comms::UnitName;
    use tokio::time::sleep;
    use tokio::time::Duration;

    use sdv_comms::id::ServiceFqin;
    use sdv_comms::ContextRef;
    use sdv_mw_rs_comms::ClientOptions;
    use sdv_mw_rs_comms::RpcClient;
    use sdv_mw_rs_sdv_comms::SdvComms;

    use com_sdv_someip_hey_interface_rpc::Client;
    use sdv_mw_rs_com_sdv_google_sample_someip_hey_service_client::HeyServiceClient;
    use sdv_someip_integration_test::run_vsomeip_server;
    use someip_messages_catalog::hey_rpc::HeyMessageRequest;

    const VSOMEIP_SERVER_BINARY: &str = "/vendor/bin/sdv_vsomeip_hey_tester";
    const VSOMEIP_CONFIG_FILE: &str = "/vendor/etc/vsomeip/hey_tester.json";

    const SERVER_START_WAIT_TIME: u64 = 3000;
    const SERVER_POLL_INTERVAL: u64 = 200;

    #[tokio::test]
    async fn test_someip_broker_rpc() {
        sdv_log::init_logger("test_someip_broker_rpc").unwrap();
        let mut server = run_vsomeip_server(VSOMEIP_SERVER_BINARY, VSOMEIP_CONFIG_FILE);
        run_sdv_rpc_client().await;
        let _ = server.kill();
        let _ = server.wait();
    }

    async fn run_sdv_rpc_client() {
        let sdv_comms = SdvComms { context: create_context() };

        let service = HeyServiceClient::new(Arc::new(sdv_comms))
            .await
            .expect("Hey Service Client should be created");
        let request1 =
            HeyMessageRequest { request: "SDV Application".to_string(), ..Default::default() };
        let request2 = HeyMessageRequest {
            request: "another SDV application".to_string(),
            ..Default::default()
        };
        // TODO(b/372453493): Avoid manual waiting for server
        let client: Client = {
            let mut counter = 0;
            let counter_end = SERVER_START_WAIT_TIME.div_ceil(SERVER_POLL_INTERVAL);
            loop {
                match service
                    .create_rpc_client(
                        UnitName::builder()
                            .package_name("com.sdv.someip")
                            .bundle_name("SomeIpBroker")
                            .service_unit_name(Client::DEFAULT_UNIT_NAME)
                            .build()
                            .unwrap(),
                        ClientOptions::builder()
                            .timeout_in_sec(3)
                            .build()
                            .expect("invalid ClientOptions"),
                    )
                    .await
                {
                    Ok(rpc_client) => break rpc_client,
                    Err(_) => {
                        sleep(Duration::from_millis(SERVER_POLL_INTERVAL)).await;
                        counter += 1;
                        if counter >= counter_end {
                            panic!("Timed out waiting for RPC server")
                        }
                    }
                }
            }
        };
        let response1 = client.SayHello(&request1).await;
        let response2 = client.SayHello(&request2).await;

        assert_eq!(response1.unwrap().response, "Hey SDV Application");
        assert_eq!(response2.unwrap().response, "Hey another SDV application");
    }

    fn create_context() -> ContextRef {
        let fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.sdv.google.sample.someip")
            .service_bundle_name("HeyServiceClient")
            .service_instance_name("instance1")
            .build()
            .unwrap();

        ContextRef::create(fqin)
    }
}
