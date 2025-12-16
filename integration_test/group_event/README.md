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

# SDV SOME/IP broker Group event integration test

This folder contains contains the source code for an integration test for the SOME/IP broker group event capability. The following scenario is tested:

* SOME/IP event split into 2 SDV topics

To run this test, launch a device via this command:
`cvd start --guest-enforce-security=false --base_instance_num=1`

then run this command:
`atest sdv_core_core_services_integration_someip_group_event_test`

## Middleware generation

This test depends on SOME/IP Vehicle Catalog definitions:
`SomeipGroupEventConsumer` from `core_services/samples/some_ip/catalog/integration_test/group_consumer.vsidl`

SOME/IP translation logic depends on the mapping in:
`core_services/samples/some_ip/catalog/integration_test/group_event_producer.vsidl`

## Test Server

The test server is the `Group Service` defined under `samples/some_ip/virtual_ecu/src/integration_test`. The server publishes an event consisting of 2 fields. The first field is a single-field struct, with a boolean field named `sample_boolean`. The second field is also a single-field struct, with an unisgned int field named `sample_int`. This event, when received by the SOME/IP broker, is split into 2 SDV topics, `FirstPart` and `SeconPart`. The server publishes the event periodically every 3 seconds.

## Test Client

This test acts as a client to the `Group Service`. The test does the following:

* The client subscribes to 2 topics `FirstPart` and `SecondPart`, which are created by the SOME/IP broker from the event published by the `Group Service`.
* The test asserts that it receives the correct expected messages (`true` for the `FirstPart` topic, `42` for the `SecondPart` topic) befor exiting successfully.
