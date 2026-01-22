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

pub(crate) mod broker;
pub(crate) mod notifier;
pub(crate) mod protocol_mapping;
pub(crate) mod service_matcher;

pub(crate) use broker::SomeIpBroker;
pub(crate) use notifier::AvailabilityMessage;
pub(crate) use notifier::ChannelMessageFactory;
pub(crate) use notifier::IAvailabilityMessageFactory;
pub(crate) use notifier::IAvailabilityNotifier;
pub(crate) use notifier::IAvailabilitySubscriber;
pub(crate) use notifier::ServiceType;

#[cfg(test)]
pub(crate) use notifier::MockIAvailabilityNotifier;
