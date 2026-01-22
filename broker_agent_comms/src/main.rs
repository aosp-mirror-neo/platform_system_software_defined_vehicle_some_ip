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

//! # SOME/IP Broker
//!
//! This agent is responsible for enabling communication between SOME/IP communication and SDV.

mod agent;
mod benchmark;
mod binder_load_indicators;
mod comms;
mod config;
mod mapping_parser;
mod pubsub;
mod rpc;
mod service_discovery;
mod translator;

use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use sdv_comms::id::ServiceFqin;
use sdv_comms::sd::UnitType;
use sdv_config_provider::{vsidl_provider, vsidl_provider_api::VsidlConfigProviderV1};
use sdv_dump::DumpBinderConnection;
use someip_config::broker_params::SdMinorVersionMode;
use someip_config::serde_config::ffi::{Endianness, SomeIpConfig};
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

use crate::agent::protocol_mapping::TypeMapping;
use crate::agent::SomeIpBroker;
use crate::benchmark::benchmark_api::IBenchmark;
use crate::benchmark::benchmark_dumpsys::BenchmarkDumpsys;
use crate::binder_load_indicators::load_indicators::LoadIndicatorsBinderConnection;
use crate::comms::SdvCommsContext;
use crate::comms::SomeIpStackContextFactoryImpl;
use crate::config::BrokerConfig;
use crate::mapping_parser::{BrokerMappingParser, IMappingParser};
use crate::pubsub::{IPubSubHandler, PubSubHandler};
use crate::rpc::{IRpcHandler, RpcHandler};
use crate::service_discovery::{IServiceDiscoveryHandler, ServiceDiscoveryHandler};
use crate::translator::{BrokerTranslator, IBrokerTranslator};
use crate::translator::{DirectTranslator, SomeIpDescriptorStore};

const TEMP_VM_NAME: &str = "vm";
const BROKER_PACKAGE: &str = "com.sdv.someip";
const BROKER_BUNDLE: &str = "SomeIpBroker";

const BROKER_CONFIG_PATH_PROP: &str = "ro.sdv.someip.broker_config";
const AVAILABILITY_MESSAGE_QUEUE_LENGTH: usize = 100;

const MINIMUM_WORKER_THREADS: usize = 4;

static BINDER_TAG: &str = "someip_broker";
static DUMP_REPORT_TITLE: &str = "SDV Agent dump - SOME/IP Broker";

static SOMEIP_REPORT_WINDOW_DELTA_T: Duration = Duration::new(5, 0);

fn main() -> anyhow::Result<()> {
    sdv_log::init_logger("someip_broker_agent").unwrap();
    sdv_tracing::init_tracing();
    let runtime = create_runtime();
    runtime.block_on(async { run_broker_agent().await })
}

async fn run_broker_agent() -> anyhow::Result<()> {
    binder::ProcessState::start_thread_pool();

    let broker_config = get_broker_config();
    let mapping_parser = create_mapping_parser(
        broker_config.serialization_config,
        broker_config.default_sd_minor_version_mode,
    );

    let fqin_builder = ServiceFqin::builder();
    let broker_fqin = fqin_builder
        .sdv_vm_name(TEMP_VM_NAME)
        .sdv_package_name(BROKER_PACKAGE)
        .service_bundle_name(BROKER_BUNDLE)
        .service_instance_name("default")
        .build()
        .map_err(|e| anyhow::anyhow!("SOME/IP broker should have a valid FQIN - Error {e:?}"))?;

    let sdv_comms_context = Arc::new(SdvCommsContext::create(broker_fqin.clone()));

    let vsidl_provider = vsidl_provider::new_for_someip_broker_agent();

    let mappings = vsidl_provider
        .get_someip_mappings(&broker_fqin)
        .await
        .ok_or(anyhow::anyhow!("Error getting vsidl provider mappings for {broker_fqin:?}"))?;
    let broker_mapping = mapping_parser.parse_mappings(mappings);

    let dm = DumpBinderConnection::initialize(DUMP_REPORT_TITLE, BINDER_TAG);
    dm.set_self_fqin(&broker_fqin);
    let benchmark_dumpsys =
        Arc::new(BenchmarkDumpsys::<UnitType>::new(SOMEIP_REPORT_WINDOW_DELTA_T));
    dm.add_reporter("SOME/IP Broker Translation Benchmark", &benchmark_dumpsys);

    let descriptor_store = SomeIpDescriptorStore::create(
        &broker_mapping.protocol_mapping,
        &broker_mapping.type_mapping,
        &vsidl_provider,
        &broker_fqin,
    )
    .await;

    let translator = create_translator(
        Box::new(vsidl_provider),
        descriptor_store,
        broker_mapping.type_mapping,
        broker_config.serialization_config,
        Some(benchmark_dumpsys),
    )?;

    let sd_handler =
        create_sd_handler(sdv_comms_context.clone(), broker_config.default_sd_minor_version_mode);
    let pubsub_handler = create_pubsub_handler(
        sdv_comms_context.clone(),
        translator.clone(),
        broker_config.dt_polling_interval,
    );
    let rpc_handler = create_rpc_handler(sdv_comms_context, translator);

    let mut broker = SomeIpBroker::new(
        broker_mapping.protocol_mapping,
        Box::new(SomeIpStackContextFactoryImpl {}),
        sd_handler,
        pubsub_handler,
        rpc_handler,
        AVAILABILITY_MESSAGE_QUEUE_LENGTH,
    );

    dm.set_agent_state("someip_broker_agent has started successfully");

    let load_indicators = LoadIndicatorsBinderConnection::initialize();

    broker.start(Some(load_indicators)).await
}

fn get_broker_config() -> BrokerConfig {
    let broker_config = rustutils::android::system_properties::read(BROKER_CONFIG_PATH_PROP)
        .expect("No valid configuration path for SOME/IP broker!");

    let broker_config_path =
        broker_config.unwrap_or_else(|| panic!("Property {BROKER_CONFIG_PATH_PROP} is not set!"));

    BrokerConfig::get_config(broker_config_path.clone().into())
        .unwrap_or_else(|e| panic!("Unable to load config {broker_config_path}, {e}"))
}

fn create_mapping_parser(
    someip_config: SomeIpConfig,
    default_sd_minor_version_mode: SdMinorVersionMode,
) -> Box<dyn IMappingParser> {
    Box::new(BrokerMappingParser::new(someip_config, default_sd_minor_version_mode))
}

fn create_translator(
    vsidl_provider: Box<dyn VsidlConfigProviderV1 + Send + Sync>,
    descriptor_store: SomeIpDescriptorStore,
    type_mapping: TypeMapping,
    someip_config: SomeIpConfig,
    benchmark: Option<Arc<dyn IBenchmark<UnitType>>>,
) -> anyhow::Result<Arc<dyn IBrokerTranslator + Send + Sync>> {
    match someip_config.endianness {
        Endianness::BigEndian => Ok(Arc::new(BrokerTranslator::new(
            vsidl_provider,
            descriptor_store,
            type_mapping,
            someip_config,
            DirectTranslator::<byteorder::BigEndian>::new(),
            benchmark,
        )) as Arc<dyn IBrokerTranslator + Send + Sync>),
        Endianness::LittleEndian => Ok(Arc::new(BrokerTranslator::new(
            vsidl_provider,
            descriptor_store,
            type_mapping,
            someip_config,
            DirectTranslator::<byteorder::LittleEndian>::new(),
            benchmark,
        )) as Arc<dyn IBrokerTranslator + Send + Sync>),
        _ => {
            bail!("Unknown endianness format")
        }
    }
}

fn create_sd_handler(
    sdv_comms_context: Arc<SdvCommsContext>,
    _default_sd_minor_version_mode: SdMinorVersionMode,
) -> Box<dyn IServiceDiscoveryHandler> {
    Box::new(ServiceDiscoveryHandler::new(sdv_comms_context))
}

fn create_rpc_handler(
    sdv_comms_context: Arc<SdvCommsContext>,
    translator: Arc<dyn IBrokerTranslator + Send + Sync>,
) -> Box<dyn IRpcHandler> {
    Box::new(RpcHandler::new(sdv_comms_context, translator))
}

fn create_pubsub_handler(
    sdv_comms_context: Arc<SdvCommsContext>,
    translator: Arc<dyn IBrokerTranslator + Send + Sync>,
    dt_polling_interval: Duration,
) -> Box<dyn IPubSubHandler> {
    Box::new(PubSubHandler::new(sdv_comms_context, translator, dt_polling_interval))
}

fn create_runtime() -> Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(std::cmp::max(MINIMUM_WORKER_THREADS, num_cpus::get()))
        .build()
        .expect("Tokio runtime should be created successfully")
}
