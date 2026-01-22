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

use std::collections::HashMap;

use google_sdv_someip::aidl::google::sdv::someip::SomeIpService::SomeIpService;

use sdv_comms::rpc::Client;
use sdv_comms::sd::ServiceUnitDefinition;

use crate::agent::protocol_mapping::RpcMethodMappingInfo;

#[derive(Debug, Clone)]
pub(crate) struct SdvRpcInterfaceInfo {
    pub someip_service_instance_info: SomeIpService,
    pub rpc_mapping_info: Vec<RpcMethodMappingInfo>,
}

impl SdvRpcInterfaceInfo {
    pub(crate) fn new(
        someip_service_instance_info: SomeIpService,
        rpc_mapping_info: Vec<RpcMethodMappingInfo>,
    ) -> Self {
        SdvRpcInterfaceInfo { someip_service_instance_info, rpc_mapping_info }
    }
}

/// Struct to hold information about a RPC Client to a SDV server
pub(crate) struct BrokerRpcClientInfo {
    /// Client transport
    pub rpc_client: Client,
    /// Unit Type
    pub service_unit_definition: Box<ServiceUnitDefinition>,
    /// Interface methods
    pub interface_methods: HashMap<u16, String>,
}

impl BrokerRpcClientInfo {
    pub fn new(
        rpc_client: Client,
        service_unit_definition: Box<ServiceUnitDefinition>,
        interface_methods: HashMap<u16, String>,
    ) -> Self {
        BrokerRpcClientInfo { rpc_client, service_unit_definition, interface_methods }
    }
}

/// This struct is used by the SOME/IP broker to match on incoming RPC messages.
/// This includes information about the service on the other end of the SOME/IP
/// connection, but excludes the minor version of the service
#[derive(Debug, Copy, Clone, PartialEq, Hash, Eq)]
pub(crate) struct BrokerSomeIpService {
    pub service_id: u16,
    pub instance_id: u16,
    pub major_version: u8,
}

impl BrokerSomeIpService {
    pub(crate) fn from_someip_service(someip_service: SomeIpService) -> Self {
        BrokerSomeIpService {
            service_id: someip_service.service_id,
            instance_id: someip_service.instance_id,
            major_version: someip_service.version.major as u8,
        }
    }
}
