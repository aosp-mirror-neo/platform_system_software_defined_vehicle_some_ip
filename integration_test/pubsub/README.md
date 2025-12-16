<!-- Copyright (C) 2026 The Android Open Source Project

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License. -->

# SDV SOME/IP broker Pub/Sub integration test

This folder contains contains the source code for an integration test for the SOME/IP broker pub/sub capability. The following 2 scenarios are tested:

* SDV topic to SOME/IP FireAndForget method
* SOME/IP event to SDV topic

To run this test, launch a device via this command:
`cvd start --guest-enforce-security=false --base_instance_num=1`

then run this command:
`atest sdv_core_core_services_integration_someip_pubsub_test`

Currently, this test can only be run once during a device's lifetime, as running it again will attempt to re-create an already existing Data Tunnel topic.

## Middleware generation

This test depends on SOME/IP Vehicle Catalog definitions:
`TestClient` from `core_services/samples/some_ip/catalog/integration_test/integration_test_system.vsidl`

SOME/IP translation logic depends on the mapping in:
`core_services/samples/some_ip/catalog/integration_test/counter_service_someip.vsidl`

## Test Server

The test server is the `Counter Service` defined under `samples/some_ip/virtual_ecu/src/integration_test`. The server waits for a Fire And Forget message with a `CounterLoad` payload to start a countdown. The `CounterLoad` message specifies the starting number for the counter and the time interval between each tick in the counter. The server publishes a `CounterLoad` message on each tick with the current count. Once the count reaches 0, the countdown stops and an `Overflow` message is published.

## Test Client

This test acts as a client to the `Counter Service`. The test does the following:
* The client subscribes to 2 topics, which are published by the `Counter Service`.
* The test client publishes a `CounterLoad` message to Data Tunnel, which the SOME/IP broker translates to a Fire and Forget method and triggers the `Counter Service` to start with the `CounterLoad` parameters.
* The test asserts that it receives the correct countdown messages from the first topic.
* The test waits to receive an overflow from the second topic before exiting successfully.
