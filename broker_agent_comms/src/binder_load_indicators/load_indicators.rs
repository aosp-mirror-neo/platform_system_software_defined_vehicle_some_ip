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

use google_sdv_someip_load_indicators::aidl::google::sdv::someip_load_indicators::LoadIndicators::LoadIndicators as LoadIndicatorsAidl;
use google_sdv_someip_load_indicators::aidl::google::sdv::someip_load_indicators::ISomeIpLoadIndicators::{
    BnSomeIpLoadIndicators, ISomeIpLoadIndicators,
};

use log::warn;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{ffi::CStr, io::Write};

const LOAD_INDICATORS_BINDER_ID: &str =
    "google.sdv.someip_load_indicators.ISomeIpLoadIndicators/default";

pub(crate) struct LoadIndicators {
    num_pending_messages: AtomicUsize,
}

impl LoadIndicators {
    pub(crate) fn new() -> Self {
        Self { num_pending_messages: AtomicUsize::new(0) }
    }

    pub(crate) fn queue_increased(&self) {
        self.num_pending_messages.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn queue_decreased(&self) {
        self.num_pending_messages.fetch_sub(1, Ordering::AcqRel);
    }

    pub(crate) fn get_current_num_pending_messages(&self) -> usize {
        self.num_pending_messages.load(Ordering::Acquire)
    }
}

struct LoadIndicatorsBinder {
    load_indicators: Arc<LoadIndicators>,
}

impl ISomeIpLoadIndicators for LoadIndicatorsBinder {
    fn getLoadIndicators(&self) -> std::result::Result<LoadIndicatorsAidl, binder::Status> {
        let curr_size = self.load_indicators.get_current_num_pending_messages() as i32;
        Ok(LoadIndicatorsAidl { pendingSomeIpEventCounter: curr_size })
    }
}
impl binder::Interface for LoadIndicatorsBinder {
    fn dump(&self, writer: &mut dyn Write, _args: &[&CStr]) -> Result<(), binder::StatusCode> {
        let _ = writeln!(writer, "Current Load: {:?}", self.getLoadIndicators());
        Ok(())
    }
}

pub struct LoadIndicatorsBinderConnection {}
impl LoadIndicatorsBinderConnection {
    pub fn initialize() -> Arc<LoadIndicators> {
        let load_indicators = Arc::new(LoadIndicators::new());
        let binder = LoadIndicatorsBinder { load_indicators: load_indicators.clone() };
        let defaults = binder::BinderFeatures::default();
        let binder_service = BnSomeIpLoadIndicators::new_binder(binder, defaults);
        let binder_id = LOAD_INDICATORS_BINDER_ID.to_owned();
        let res = binder::add_service(&binder_id, binder_service.as_binder());
        if let Err(e) = res {
            warn!("LoadIndicatorsBinderConnection binder::add_service Error: {e:#?}");
        }
        load_indicators
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_indicators_new() {
        let li = LoadIndicators::new();
        assert_eq!(li.get_current_num_pending_messages(), 0);
    }

    #[test]
    fn test_load_indicators_queue_increased() {
        let li = LoadIndicators::new();
        li.queue_increased();
        assert_eq!(li.get_current_num_pending_messages(), 1);
        li.queue_increased();
        assert_eq!(li.get_current_num_pending_messages(), 2);
    }

    #[test]
    fn test_load_indicators_queue_decreased() {
        let li = LoadIndicators::new();
        li.queue_increased(); // val is 1
        li.queue_increased(); // val is 2
        li.queue_decreased();
        assert_eq!(li.get_current_num_pending_messages(), 1);
        li.queue_decreased();
        assert_eq!(li.get_current_num_pending_messages(), 0);
    }
}
