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

* SDV RPC being called by SOME/IP service

To run this test, launch a device via this command:
`cvd start --guest-enforce-security=false --base_instance_num=1`

then run this command:
`atest sdv_core_core_services_integration_someip_sdv_rpc_test`

## Middleware generation

This test depends on SOME/IP Vehicle Catalog definitions:
`SdvIntegrationTestServer` from `core_services/samples/some_ip/catalog/integration_test/test_hello_service.vsidl`

SOME/IP translation logic depends on the mapping in:
`core_services/samples/some_ip/catalog/integration_test/test_hello_service_sdv_someip.vsidl`

## Test Server

This test acts as the test service server. The server offers a RPC method, `sdv.someip.integration_test.TestService.SayHello`. This method is exposed on SOME/IP as the RPC method with ID `5000`. The method accepts a String parameter, prepends the string "SDV Hello " to it and returns the new text.

## Test Client

The test client is the `SDV Hello Tester` defined under `samples/some_ip/virtual_ecu/src/integration_test`. The tester calls the `SayHello` once, and checks that the response is received. If the response is as expected, the tester exits normally with a status code of 0, otherwise it exits with a status code of 1.
