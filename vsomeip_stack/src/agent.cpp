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

#include <aidl/google/sdv/someip/ISomeIpStack.h>
#include <aidl/google/sdv/someip_load_indicators/BnSomeIpLoadIndicators.h>
#include <aidl/google/sdv/someip_load_indicators/ISomeIpLoadIndicators.h>
#include <aidl/google/sdv/someip_load_indicators/LoadIndicators.h>
#include <android/binder_manager.h>
#include <android/binder_process.h>
#include <sdv/logging.h>

#include <chrono>
#include <thread>

constexpr char kSomeIpStackAgent[] = "google.sdv.someip.ISomeIpStack/default";

int main() {
    LOG(INFO) << "SOME/IP STACK - Starting ...";

    auto vsomeip_stack = ndk::SharedRefBase::make<android::sdv::someip::StackImpl>();
    if (!vsomeip_stack->init()) {
        LOG(FATAL) << "SOME/IP STACK - Couldn't initialize application";
        return -1;
    }
    std::thread thread([&vsomeip_stack] { vsomeip_stack->start(); });
    bool stop_load_indicators_thread = false;
    std::thread load_indicators_thread([&stop_load_indicators_thread] {
        std::string serviceName =
                aidl::google::sdv::someip_load_indicators::BnSomeIpLoadIndicators::descriptor;
        serviceName += "/default";
        if (!AServiceManager_isDeclared(serviceName.c_str())) {
            LOG(ERROR) << serviceName << " is not declared";
            return;
        }
        ndk::SpAIBinder serviceBinder(AServiceManager_waitForService(serviceName.data()));
        auto service =
                aidl::google::sdv::someip_load_indicators::BnSomeIpLoadIndicators::fromBinder(
                        serviceBinder);
        if (service == nullptr) {
            LOG(ERROR) << "Cannot find service " << serviceName;
            return;
        }

        while (!stop_load_indicators_thread) {
            ::aidl::google::sdv::someip_load_indicators::LoadIndicators load_indicators;
            service->getLoadIndicators(&load_indicators);
            LOG(INFO) << "someip_load_indicators value = "
                      << load_indicators.pendingSomeIpEventCounter;
            std::this_thread::sleep_for(std::chrono::seconds(5));
        }
    });
    ABinderProcess_startThreadPool();
    auto status = AServiceManager_addService(vsomeip_stack->asBinder().get(), kSomeIpStackAgent);
    if (status != EX_NONE) {
        LOG(FATAL) << "Error while registering vSOME/IP stack: "
                   << ::android::statusToString(status);
        return -1;
    }
    ABinderProcess_joinThreadPool();
    thread.join();

    stop_load_indicators_thread = true;
    load_indicators_thread.join();

    return 0;
}
