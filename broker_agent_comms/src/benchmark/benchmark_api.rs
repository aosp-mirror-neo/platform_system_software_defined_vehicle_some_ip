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
use std::fmt::Display;
use std::hash::Hash;
use tokio::time::Instant;

pub(crate) trait IBenchmarkArgument:
    Hash + Sync + Send + Display + Eq + PartialEq + Clone
{
}
impl<T: Hash + Sync + Send + Display + Eq + PartialEq + Clone> IBenchmarkArgument for T {}

pub(crate) trait IBenchmark<T: IBenchmarkArgument>: Send + Sync {
    /// Adds a measurement
    fn add_measurement(&self, key: &T, start: Instant, stop: Instant);
}
