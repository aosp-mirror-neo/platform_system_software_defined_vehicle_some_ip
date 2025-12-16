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

use log::error;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;

use google_sdv_someip::{
    aidl::google::sdv::someip::{
        IRpcRequestCallback::IRpcRequestCallbackAsyncServer, SomeIpService::SomeIpService,
    },
    binder::{self, StatusCode},
};

use crate::translator::{IBrokerTranslator, TranslationMode};
use crate::{
    rpc::config::{BrokerRpcClientInfo, BrokerSomeIpService},
    translator::{RpcMode, SomeIpDescriptorId},
};

pub(crate) struct RpcRequestCallback {
    broker_rpc_client_map: Arc<DashMap<BrokerSomeIpService, BrokerRpcClientInfo>>,
    translator: Arc<dyn IBrokerTranslator>,
}

impl RpcRequestCallback {
    pub(crate) fn create(
        broker_rpc_client_map: Arc<DashMap<BrokerSomeIpService, BrokerRpcClientInfo>>,
        translator: Arc<dyn IBrokerTranslator>,
    ) -> Self {
        RpcRequestCallback { broker_rpc_client_map, translator }
    }
}

impl binder::Interface for RpcRequestCallback {}

#[async_trait]
impl IRpcRequestCallbackAsyncServer for RpcRequestCallback {
    async fn onRpcRequest(
        &self,
        someip_service: &SomeIpService,
        method_id: u16,
        payload: &[u8],
    ) -> binder::Result<Vec<u8>> {
        let Some(rpc_info) = self
            .broker_rpc_client_map
            .get(&BrokerSomeIpService::from_someip_service(*someip_service))
        else {
            error!("Received RPC request for unknown service {someip_service:?}");
            return Err(StatusCode::UNKNOWN_ERROR.into());
        };
        // We have to call the `Option::cloned`` method to avoid holding onto a reference inside the map
        let Some(rpc_method_name) = rpc_info.value().interface_methods.get(&method_id).cloned()
        else {
            error!("Received RPC request for unknown method ID {method_id} on service {someip_service:?}");
            return Err(StatusCode::UNKNOWN_ERROR.into());
        };
        let descriptor_id = SomeIpDescriptorId::new_method_id(
            someip_service.service_id,
            someip_service.version.major as u8,
            method_id,
            RpcMode::Request,
        );
        let proto_format_data = self
            .translator
            .translate_from_someip_to_protobuf_rpc(
                &rpc_info.value().service_unit_definition.get_unit_type(),
                payload,
                &descriptor_id,
            )
            .await
            .map_err(|e| {
                error!("Error translating SOME/IP request to SDV format: {e:?}");
                StatusCode::UNKNOWN_ERROR
            })?;

        let response = rpc_info
            .value()
            .rpc_client
            .unary_call(&rpc_method_name, proto_format_data.get_data())
            .await
            .map_err(|e| {
                error!("Error communicating via RPC with SDV service: {e:?}");
                StatusCode::UNKNOWN_ERROR
            })?;

        self.translator
            .translate_from_protobuf_to_someip(
                rpc_info.value().service_unit_definition.get_service_bundle_identity().get_fqin(),
                &rpc_info.value().service_unit_definition.get_unit_type(),
                &response,
                TranslationMode::RpcResponse(rpc_method_name.to_string()),
            )
            .await
            .map_err(|e| {
                error!("Error translating SDV response to SOME/IP format: {e:?}");
                StatusCode::UNKNOWN_ERROR.into()
            })
    }
}
