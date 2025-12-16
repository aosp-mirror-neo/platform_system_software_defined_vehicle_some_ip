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

# SDV SOME/IP broker RPC integration test

This folder contains contains the source code for an integration test for the SOME/IP broker RPC capability. The following scenarios is tested:

* SOME/IP RPC being called by SDV service

To run this test, launch a device via this command:
`cvd start --guest-enforce-security=false --base_instance_num=1`

then run this command:
`atest sdv_core_core_services_integration_someip_rpc_test`

## Middleware generation

This test depends on SOME/IP Vehicle Catalog definitions:
`HeyServiceClient` from `core_services/samples/some_ip/catalog/integration_test/integration_test_system.vsidl`

SOME/IP translation logic depends on the mapping in:
`core_services/samples/some_ip/catalog/integration_test/counter_service_someip.vsidl`

## Test Server

The test server is the `Hey Service` defined under `samples/some_ip/virtual_ecu/src/integration_test`. The server offers a RPC method with ID `0x1000`. This method is exposed on SDV as the RPC method `sdv.someip.integration_test.HeyInterface.SayHello`. The method accepts a String parameter, prepends the string "Hey " to it and returns the new text.

## Test Client

This test acts as a client to the `Hey Service`. The test calls the `SayHello` method twice with different arguments. It then asserts that the responses received are as exepcted ("Hey " + argument text)
