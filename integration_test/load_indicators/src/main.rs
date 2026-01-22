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
    use google_sdv_someip_load_indicators::aidl::google::sdv::someip_load_indicators::ISomeIpLoadIndicators::ISomeIpLoadIndicators;

    const LOAD_INDICATORS_BINDER_ID: &str =
        "google.sdv.someip_load_indicators.ISomeIpLoadIndicators/default";

    #[test]
    fn test_load_indicators_binder_interface_ok() {
        binder::ProcessState::start_thread_pool();
        let service = binder::get_interface::<dyn ISomeIpLoadIndicators>(LOAD_INDICATORS_BINDER_ID)
            .map_err(|_| "Failed to connect to binder service")
            .expect("REASON");

        // Call the service.
        let binder_result = service.getLoadIndicators();

        assert!(binder_result.is_ok());
    }

    #[test]
    fn test_load_indicators_binder_interface_value() {
        binder::ProcessState::start_thread_pool();
        let service = binder::get_interface::<dyn ISomeIpLoadIndicators>(LOAD_INDICATORS_BINDER_ID)
            .map_err(|_| "Failed to connect to binder service")
            .expect("REASON");

        // Call the service.
        let binder_result = service.getLoadIndicators();

        assert!(binder_result.as_ref().unwrap().pendingSomeIpEventCounter >= 0);
    }
}
