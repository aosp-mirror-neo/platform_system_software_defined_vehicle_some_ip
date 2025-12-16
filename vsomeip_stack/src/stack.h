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

#include <aidl/google/sdv/someip/AvailabilityStatus.h>
#include <aidl/google/sdv/someip/BnSomeIpStack.h>
#include <aidl/google/sdv/someip/IRpcRequestCallback.h>
#include <aidl/google/sdv/someip/ISomeIpServiceAvailabilityCallback.h>
#include <aidl/google/sdv/someip/SomeIpService.h>
#include <aidl/google/sdv/someip/SomeIpServiceVersion.h>

#include <algorithm>
#include <condition_variable>
#include <mutex>
#include <unordered_map>
#include <unordered_set>

#include <vsomeip/vsomeip.hpp>

namespace aidl_namespace = ::aidl::google::sdv::someip;

namespace android::sdv::someip {

struct SomeIpService {
    aidl_namespace::SomeIpService service_impl;

    bool operator==(const SomeIpService& other) const {
        return service_impl.service_id == other.service_impl.service_id &&
                service_impl.instance_id == other.service_impl.instance_id &&
                service_impl.version.major == other.service_impl.version.major &&
                service_impl.version.minor == other.service_impl.version.minor;
    }
};

struct SomeIpServiceHash {
    size_t operator()(SomeIpService service) const noexcept {
        auto hash_value = 17;
        hash_value *= 31;
        hash_value += std::hash<uint16_t>()(service.service_impl.service_id);
        hash_value *= 31;
        hash_value += std::hash<uint16_t>()(service.service_impl.instance_id);
        hash_value *= 31;
        hash_value += std::hash<uint8_t>()(service.service_impl.version.major);
        hash_value *= 31;
        hash_value += std::hash<uint32_t>()(service.service_impl.version.minor);
        return hash_value;
    }
};

struct SomeIpEvent {
    SomeIpService publishing_service;
    char16_t event_id;

    bool operator==(const SomeIpEvent& other) const {
        return publishing_service == other.publishing_service && event_id == other.event_id;
    }
};

struct SomeIpEventHash {
    size_t operator()(SomeIpEvent event) const noexcept {
        auto hash_value = 17;
        hash_value *= 31;
        hash_value += SomeIpServiceHash{}(event.publishing_service);
        hash_value *= 31;
        hash_value += std::hash<char16_t>()(event.event_id);
        return hash_value;
    }
};

struct SomeIpMessageRequest {
    SomeIpService service;
    char16_t method_id;
    uint32_t request_id;

    bool operator==(const SomeIpMessageRequest& other) const {
        return service == other.service && method_id == other.method_id &&
                request_id == other.request_id;
    }
};

struct SomeIpMessageRequestHash {
    size_t operator()(SomeIpMessageRequest request) const noexcept {
        auto hash_value = 17;
        hash_value *= 31;
        hash_value += SomeIpServiceHash{}(request.service);
        hash_value *= 31;
        hash_value += std::hash<char16_t>()(request.method_id);
        hash_value *= 31;
        hash_value += std::hash<uint32_t>()(request.request_id);
        return hash_value;
    }
};

class StackImpl : public aidl::google::sdv::someip::BnSomeIpStack {
    struct RpcHandler {
        std::mutex mutex;
        std::condition_variable cv;
        std::optional<std::vector<uint8_t>> response;
    };

public:
    StackImpl();

    bool init();

    void start();

    ndk::ScopedAStatus monitor_service(
            const aidl_namespace::SomeIpService& someip_service) override;

    ndk::ScopedAStatus rpc_transact(const aidl_namespace::SomeIpService& someip_service,
                                    const char16_t method_id,
                                    const std::vector<uint8_t>& message_payload,
                                    std::vector<uint8_t>* out) override;

    ndk::ScopedAStatus rpc_oneway(const aidl_namespace::SomeIpService& someip_service,
                                  const char16_t method_id,
                                  const std::vector<uint8_t>& message_payload) override;

    ndk::ScopedAStatus register_availability_callback(
            const std::shared_ptr<aidl_namespace::ISomeIpServiceAvailabilityCallback>&
                    availability_callback) override;

    ndk::ScopedAStatus unregister_availability_callback() override;

    ndk::ScopedAStatus register_rpc_request_callback(
            const std::shared_ptr<aidl_namespace::IRpcRequestCallback>& callback);

    ndk::ScopedAStatus unregister_rpc_request_callback();

    ndk::ScopedAStatus register_event_notification_callback(
            const std::shared_ptr<aidl_namespace::IEventNotificationCallback>&
                    event_notification_callback) override;

    ndk::ScopedAStatus unregister_event_notification_callback() override;

    ndk::ScopedAStatus on_sdv_service_availability(
            const aidl_namespace::SomeIpService& someip_service,
            aidl_namespace::AvailabilityStatus status) override;

    ndk::ScopedAStatus subscribe(const aidl_namespace::SomeIpService& someip_service,
                                 char16_t eventgroup_id, char16_t event_id) override;

    ndk::ScopedAStatus unsubscribe(const aidl_namespace::SomeIpService& someip_service,
                                   char16_t eventgroup, char16_t event_id) override;

    ndk::ScopedAStatus publish(const aidl_namespace::SomeIpService& someip_service,
                               char16_t event_id,
                               const std::vector<uint8_t>& event_payload) override;

    ndk::ScopedAStatus on_sdv_topic_availability(
            const aidl_namespace::SomeIpService& someip_service, char16_t eventgroup,
            char16_t event, aidl_namespace::AvailabilityStatus status) override;

    ndk::ScopedAStatus subscribe_eventgroup(
            const ::aidl::google::sdv::someip::SomeIpService& someip_service,
            char16_t eventgroup_id, const std::vector<char16_t>& event_ids) override;

    ndk::ScopedAStatus unsubscribe_eventgroup(
            const ::aidl::google::sdv::someip::SomeIpService& in_service, char16_t in_eventgroup,
            const std::vector<char16_t>& in_event_ids) override;

private:
    aidl_namespace::SomeIpService create_someip_service_id(
            const std::shared_ptr<vsomeip::message>& message);

    SomeIpEvent create_someip_event(const std::shared_ptr<vsomeip::message>& message);

    SomeIpMessageRequest getSomeIpMessageRequest(const std::shared_ptr<vsomeip::message>& message);

    void lazyRequestService(const aidl_namespace::SomeIpService& service);

    void onResponse(const std::shared_ptr<vsomeip::message>& message);

    void onServiceAvailable(vsomeip::service_t service_id, vsomeip::instance_t instance_id,
                            bool available);

    void changeServiceOffer(const aidl_namespace::SomeIpService& service,
                            const aidl_namespace::AvailabilityStatus status);

    // Properties
    std::shared_ptr<vsomeip::runtime> runtime;
    std::shared_ptr<vsomeip::application> app;

    std::mutex sd_mutex;
    std::shared_ptr<aidl_namespace::ISomeIpServiceAvailabilityCallback> availability_callback;

    // map for rpc requests waiting for responses.
    std::mutex rpc_mutex;
    std::unordered_map<SomeIpMessageRequest, std::shared_ptr<RpcHandler>, SomeIpMessageRequestHash>
            rpc_requests;

    // set for subscribed events.
    std::mutex pub_sub_mutex;
    std::unordered_set<SomeIpEvent, SomeIpEventHash> subscribed_events;

    // rpc request callback to be triggered when rpc requests are received.
    std::mutex rpc_request_mutex;
    std::shared_ptr<aidl_namespace::IRpcRequestCallback> rpc_request_callback;

    // event notification callback to be triggered when event notifications are received.
    std::mutex event_notification_mutex;
    std::shared_ptr<aidl_namespace::IEventNotificationCallback> event_notification_callback;

    std::mutex requested_instances_mutex;
    std::unordered_set<SomeIpService, SomeIpServiceHash> requested_instances;

    std::mutex offered_instances_mutex;
    std::unordered_set<SomeIpService, SomeIpServiceHash> offered_instances;
};

}  // namespace android::sdv::someip
