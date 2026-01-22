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

#include "stack.h"

#include "aidl/google/sdv/someip/SomeIpService.h"

namespace android::sdv::someip {

StackImpl::StackImpl() :
      runtime(vsomeip::runtime::get()), app(runtime->create_application("SampleStack")) {}

bool StackImpl::init() {
    if (!app->init()) {
        return false;
    }
    app->register_message_handler(vsomeip::ANY_SERVICE, vsomeip::ANY_INSTANCE, vsomeip::ANY_METHOD,
                                  std::bind(&StackImpl::onResponse, this, std::placeholders::_1));

    // register a callback which is called as soon as a service is available
    app->register_availability_handler(vsomeip::ANY_SERVICE, vsomeip::ANY_INSTANCE,
                                       std::bind(&StackImpl::onServiceAvailable, this,
                                                 std::placeholders::_1, std::placeholders::_2,
                                                 std::placeholders::_3),
                                       vsomeip::ANY_MAJOR, vsomeip::ANY_MINOR);

    return true;
}

void StackImpl::start() {
    app->start();
}

ndk::ScopedAStatus StackImpl::monitor_service(const aidl_namespace::SomeIpService& someip_service) {
    lazyRequestService(someip_service);
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::rpc_transact(const aidl_namespace::SomeIpService& someip_service,
                                           const char16_t method_id,
                                           const std::vector<uint8_t>& message_payload,
                                           std::vector<uint8_t>* out) {
    lazyRequestService(someip_service);
    std::shared_ptr<vsomeip::message> request = runtime->create_request(/*use_tcp=*/true);
    request->set_service(someip_service.service_id);
    request->set_instance(someip_service.instance_id);
    request->set_interface_version(someip_service.version.major);
    request->set_method(method_id);

    // Create a payload which will be sent to the service.
    std::shared_ptr<vsomeip::payload> payload = runtime->create_payload();

    payload->set_data(message_payload);
    request->set_payload(payload);

    std::shared_ptr<StackImpl::RpcHandler> handler = std::make_shared<StackImpl::RpcHandler>();
    {
        // Atomically send the message to the service and register a RPC response handler. The
        // 'send' method fills the request id of the passed in message parameter. Any response
        // will be delivered to the registered message handler.
        std::lock_guard<std::mutex> guard(rpc_mutex);
        app->send(request);
        auto message_request = getSomeIpMessageRequest(request);
        rpc_requests[message_request] = handler;
    }
    {
        std::unique_lock cv_lock(handler->mutex);
        handler->cv.wait(cv_lock, [&handler] { return handler->response.has_value(); });
    }
    *out = std::move(handler->response.value());
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::rpc_oneway(const aidl_namespace::SomeIpService& someip_service,
                                         const char16_t method_id,
                                         const std::vector<uint8_t>& message_payload) {
    lazyRequestService(someip_service);
    std::shared_ptr<vsomeip::message> message = runtime->create_message(/*use_tcp=*/true);
    message->set_message_type(vsomeip_v3::message_type_e::MT_REQUEST_NO_RETURN);
    message->set_service(someip_service.service_id);
    message->set_instance(someip_service.instance_id);
    message->set_interface_version(someip_service.version.major);
    message->set_method(method_id);

    // Create a payload which will be sent to the service
    std::shared_ptr<vsomeip::payload> payload = runtime->create_payload();
    std::vector<vsomeip::byte_t> payload_data(std::begin(message_payload),
                                              std::end(message_payload));

    payload->set_data(payload_data);
    message->set_payload(payload);
    // Send the message to the service.
    app->send(message);
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::register_availability_callback(
        const std::shared_ptr<aidl_namespace::ISomeIpServiceAvailabilityCallback>&
                availability_callback) {
    std::lock_guard<std::mutex> guard(sd_mutex);
    this->availability_callback = availability_callback;
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::unregister_availability_callback() {
    std::lock_guard<std::mutex> guard(sd_mutex);
    this->availability_callback = nullptr;
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::register_rpc_request_callback(
        const std::shared_ptr<aidl_namespace::IRpcRequestCallback>& rpc_request_callback) {
    std::lock_guard<std::mutex> guard(rpc_request_mutex);
    this->rpc_request_callback = rpc_request_callback;
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::unregister_rpc_request_callback() {
    std::lock_guard<std::mutex> guard(rpc_request_mutex);
    this->rpc_request_callback = nullptr;
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::register_event_notification_callback(
        const std::shared_ptr<aidl_namespace::IEventNotificationCallback>&
                event_notification_callback) {
    std::lock_guard<std::mutex> guard(event_notification_mutex);
    this->event_notification_callback = event_notification_callback;
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::unregister_event_notification_callback() {
    std::lock_guard<std::mutex> guard(event_notification_mutex);
    this->event_notification_callback = nullptr;
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::on_sdv_service_availability(
        const aidl_namespace::SomeIpService& someip_service,
        aidl_namespace::AvailabilityStatus status) {
    changeServiceOffer(someip_service, status);
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::subscribe(const aidl_namespace::SomeIpService& someip_service,
                                        char16_t eventgroup_id, char16_t event_id) {
    const std::vector<char16_t> event_ids = {event_id};
    return subscribe_eventgroup(someip_service, eventgroup_id, event_ids);
}

ndk::ScopedAStatus StackImpl::unsubscribe(const aidl_namespace::SomeIpService& someip_service,
                                          char16_t eventgroup, char16_t event_id) {
    const std::vector<char16_t> event_ids = {event_id};
    return unsubscribe_eventgroup(someip_service, eventgroup, event_ids);
}

ndk::ScopedAStatus StackImpl::publish(const aidl_namespace::SomeIpService& someip_service,
                                      char16_t event_id,
                                      const std::vector<uint8_t>& event_payload) {
    std::shared_ptr<vsomeip::payload> payload = runtime->create_payload();
    payload->set_data(event_payload);

    app->notify(someip_service.service_id, someip_service.instance_id, event_id, payload);
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::on_sdv_topic_availability(
        const aidl_namespace::SomeIpService& someip_service, char16_t eventgroup, char16_t event,
        aidl_namespace::AvailabilityStatus status) {
    changeServiceOffer(someip_service, status);
    if (status == aidl_namespace::AvailabilityStatus::AVAILABLE) {
        std::set<vsomeip::eventgroup_t> eventgroups;
        eventgroups.insert(eventgroup);
        app->offer_event(someip_service.service_id, someip_service.instance_id, event, eventgroups,
                         vsomeip::event_type_e::ET_EVENT, std::chrono::milliseconds::zero(),
                         /*change_resets_cycle=*/false,
                         /*update_on_change=*/true, /*epsilon_change_func=*/nullptr,
                         vsomeip::reliability_type_e::RT_RELIABLE);
    } else {
        app->stop_offer_event(someip_service.service_id, someip_service.instance_id, event);
    }
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::subscribe_eventgroup(
        const ::aidl::google::sdv::someip::SomeIpService& someip_service, char16_t eventgroup_id,
        const std::vector<char16_t>& event_ids) {
    lazyRequestService(someip_service);
    std::unordered_set<char16_t> events;
    for (auto event_id : event_ids) {
        {
            std::lock_guard<std::mutex> guard(pub_sub_mutex);
            SomeIpService service;
            service.service_impl = someip_service;
            SomeIpEvent event;
            event.publishing_service = service;
            event.event_id = event_id;
            auto [_, inserted] = subscribed_events.insert(event);
            if (inserted) {
                events.insert(event_id);
                app->request_event(someip_service.service_id, someip_service.instance_id, event_id,
                                   {eventgroup_id}, vsomeip::event_type_e::ET_EVENT,
                                   vsomeip::reliability_type_e::RT_RELIABLE);
            }
        }
    }
    for (auto event_id : events) {
        app->subscribe(someip_service.service_id, someip_service.instance_id, eventgroup_id,
                       someip_service.version.major, event_id);
    }
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus StackImpl::unsubscribe_eventgroup(
        const ::aidl::google::sdv::someip::SomeIpService& someip_service, char16_t eventgroup_id,
        const std::vector<char16_t>& event_ids) {
    for (auto event_id : event_ids) {
        {
            std::lock_guard<std::mutex> guard(pub_sub_mutex);
            SomeIpService service;
            service.service_impl = someip_service;
            SomeIpEvent event;
            event.publishing_service = service;
            event.event_id = event_id;
            subscribed_events.erase(event);
        }
        app->unsubscribe(someip_service.service_id, someip_service.instance_id, eventgroup_id,
                         event_id);
    }
    return ndk::ScopedAStatus::ok();
}

void StackImpl::lazyRequestService(const aidl_namespace::SomeIpService& someip_service) {
    bool alreadyRequested = false;
    {
        std::lock_guard<std::mutex> guard(requested_instances_mutex);
        SomeIpService service;
        service.service_impl = someip_service;
        auto [_, inserted] = requested_instances.insert(service);
        alreadyRequested = !inserted;
    }
    if (!alreadyRequested) {
        app->request_service(someip_service.service_id, someip_service.instance_id,
                             someip_service.version.major, someip_service.version.minor);
    }
}

void StackImpl::changeServiceOffer(const aidl_namespace::SomeIpService& someip_service,
                                   const aidl_namespace::AvailabilityStatus status) {
    SomeIpService service;
    service.service_impl = someip_service;
    bool alreadyOffered = false;
    if (status == aidl_namespace::AvailabilityStatus::AVAILABLE) {
        {
            std::lock_guard<std::mutex> guard(offered_instances_mutex);
            auto [_, inserted] = offered_instances.insert(service);
            alreadyOffered = !inserted;
        }
        if (!alreadyOffered) {
            app->offer_service(someip_service.service_id, someip_service.instance_id,
                               someip_service.version.major, someip_service.version.minor);
        }
    } else {
        {
            std::lock_guard<std::mutex> guard(offered_instances_mutex);
            alreadyOffered = offered_instances.erase(service) != 0;
        }
        if (alreadyOffered) {
            app->stop_offer_service(someip_service.service_id, someip_service.instance_id,
                                    someip_service.version.major, someip_service.version.minor);
        }
    }
}

void StackImpl::onResponse(const std::shared_ptr<vsomeip::message>& message) {
    auto type = message->get_message_type();

    auto payload = message->get_payload();
    std::vector<uint8_t> payload_vector;
    std::copy(payload->get_data(), payload->get_data() + payload->get_length(),
              back_inserter(payload_vector));

    if (type == vsomeip::message_type_e::MT_NOTIFICATION) {
        std::unique_lock subscribed_set_lock(pub_sub_mutex);
        auto someip_event = create_someip_event(message);
        if (auto it = subscribed_events.find(someip_event); it != subscribed_events.end()) {
            subscribed_set_lock.unlock();

            {
                // Currently this mutex prevents shared access to 'event_notification_callback', a
                // better implementation might consider using a shared mutex to allow concurrent
                // calls
                std::lock_guard<std::mutex> guard(event_notification_mutex);
                if (event_notification_callback != nullptr) {
                    event_notification_callback
                            ->onEvent(someip_event.publishing_service.service_impl,
                                      someip_event.event_id, payload_vector);
                }
            }
        }
    } else if (type == vsomeip::message_type_e::MT_RESPONSE) {
        std::lock_guard<std::mutex> guard(rpc_mutex);
        auto message_request = getSomeIpMessageRequest(message);
        if (auto handler = rpc_requests.find(message_request); handler != rpc_requests.end()) {
            rpc_requests.erase(message_request);
            {
                std::unique_lock cv_lock(handler->second->mutex);
                handler->second->response.emplace(std::move(payload_vector));
                handler->second->cv.notify_one();
            }
        }
    } else if (type == vsomeip::message_type_e::MT_REQUEST) {
        // Consider filtering requests for services that haven't been offered
        aidl_namespace::SomeIpService service = create_someip_service_id(message);
        char16_t method_id = message->get_method();

        std::vector<uint8_t> response_bytes;
        bool trigger_request;
        {
            // Currently this mutex prevents shared access to 'rpc_request_callback', a better
            // implementation might consider using a shared mutex to allow concurrent calls
            std::lock_guard<std::mutex> guard(rpc_request_mutex);
            trigger_request = rpc_request_callback != nullptr;
            if (trigger_request) {
                rpc_request_callback->onRpcRequest(service, method_id, payload_vector,
                                                   &response_bytes);
            }
        }
        std::shared_ptr<vsomeip::message> response = runtime->create_response(message);
        if (trigger_request) {
            std::shared_ptr<vsomeip::payload> response_payload = runtime->create_payload();
            response_payload->set_data(response_bytes);
            response->set_payload(response_payload);
        } else {
            response->set_message_type(vsomeip_v3::message_type_e::MT_ERROR);
            response->set_return_code(vsomeip_v3::return_code_e::E_NOT_OK);
        }
        app->send(response);
    }
}

SomeIpMessageRequest StackImpl::getSomeIpMessageRequest(
        const std::shared_ptr<vsomeip::message>& message) {
    SomeIpService service;
    service.service_impl = create_someip_service_id(message);

    SomeIpMessageRequest request;
    request.service = service;
    request.method_id = message->get_method();
    request.request_id = message->get_request();

    return request;
}

aidl_namespace::SomeIpService StackImpl::create_someip_service_id(
        const std::shared_ptr<vsomeip::message>& message) {
    aidl_namespace::SomeIpService someip_service;
    someip_service.service_id = message->get_service();
    someip_service.instance_id = message->get_instance();

    aidl_namespace::SomeIpServiceVersion version;
    version.major = message->get_interface_version();
    version.minor = 0;

    someip_service.version = version;
    return someip_service;
}

SomeIpEvent StackImpl::create_someip_event(const std::shared_ptr<vsomeip::message>& message) {
    SomeIpService service;
    service.service_impl = create_someip_service_id(message);

    SomeIpEvent event;
    event.publishing_service = service;
    event.event_id = message->get_method();
    return event;
}

void StackImpl::onServiceAvailable(vsomeip::service_t service_id, vsomeip::instance_t instance_id,
                                   bool available) {
    std::lock_guard<std::mutex> guard(sd_mutex);
    if (availability_callback != nullptr) {
        aidl_namespace::SomeIpService someip_service;
        someip_service.service_id = service_id;
        someip_service.instance_id = instance_id;

        aidl_namespace::SomeIpServiceVersion version;
        version.major = 1;
        version.minor = 0;

        someip_service.version = version;
        if (available) {
            availability_callback
                    ->onAvailabilityChanged(someip_service,
                                            aidl_namespace::AvailabilityStatus::AVAILABLE);
        } else {
            availability_callback
                    ->onAvailabilityChanged(someip_service,
                                            aidl_namespace::AvailabilityStatus::UNAVAILABLE);
        }
    }
}
}  // namespace android::sdv::someip
