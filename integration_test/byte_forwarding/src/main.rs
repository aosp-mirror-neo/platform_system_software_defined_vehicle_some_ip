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

    use futures::StreamExt;

    use log::{debug, info};

    use sdv::mw::SdvComms;
    use std::sync::Arc;

    use sdv::comms::id::ServiceFqin;
    use sdv::comms::ContextRef;
    use sdv::mw::{Availability, SubscribeOptions};

    use sdv_mw_rs_com_android_sdv_sample_someip_object_sensor_consumer::{
        subscriber::Variant, ObjectSensorConsumer,
    };
    use sdv_someip_integration_test::run_vsomeip_server;
    use someip_messages_catalog::byte_forward::{ObjectSensor, ObjectSensorData, ObjectType};
    const VSOMEIP_SERVER_BINARY: &str = "/vendor/bin/sdv_vsomeip_object_sensor_tester";
    const VSOMEIP_CONFIG_FILE: &str = "/vendor/etc/vsomeip/object_sensor.json";

    #[tokio::test]
    async fn test_someip_broker_byte_forwarding() {
        sdv_log::init_logger("test_byte_forwarding").unwrap();

        let mut server = run_vsomeip_server(VSOMEIP_SERVER_BINARY, VSOMEIP_CONFIG_FILE);

        run_sdv_client().await;

        let _ = server.kill();
        let _ = server.wait();
    }

    async fn run_sdv_client() {
        binder::ProcessState::start_thread_pool();

        let context = create_context();
        let sdv_comms = SdvComms { context };

        let mut object_sensor_consumer =
            ObjectSensorConsumer::new(Arc::new(sdv_comms)).await.expect("Test Client should start");
        debug!("Client started");

        let mut availability_event_stream = object_sensor_consumer
            .create_registration_event_stream(
                Variant::<ObjectSensor>::UNIQUE,
                &SubscribeOptions::default(),
            )
            .await
            .expect("Availability event stream should be created");

        loop {
            match availability_event_stream.next().await {
                Some(Availability::Available) => {
                    info!("Object sensor publisher is now available");
                    break;
                }
                Some(Availability::Unavailable) => {
                    info!("Object sensor publisher unregistered, waiting for its availability");
                }
                None => {
                    panic!("Availability event stream unexpectedly ended");
                }
            }
        }

        let mut object_sensor_observer = object_sensor_consumer
            .create_observer(
                Variant::<ObjectSensor>::UNIQUE,
                &SubscribeOptions::builder()
                    .read_on_subscribe(true)
                    .min_interval(std::time::Duration::from_secs(1))
                    .build()
                    .expect("Subscribe optons should be created"),
            )
            .await
            .expect("Observer must be created");
        debug!("Publisher registered");

        let reference_message = create_reference_message();
        debug!("Awaiting messages");

        match object_sensor_observer.next().await {
            Some(Ok(messages)) => {
                for message in messages {
                    let value = deserialize_someip_bytes(&message.data);
                    assert!(sensors_match(&value, &reference_message));
                }
            }
            Some(Err(e)) => {
                panic!("Error while deserializing object sensor message(s) - {e:?}");
            }
            _ => {
                panic!("Got none on object sensor observer");
            }
        }
    }

    fn sensors_match(actual: &ObjectSensorData, expected: &ObjectSensorData) -> bool {
        if actual.object_type != expected.object_type
            && actual.distance != expected.distance
            && actual.relative != expected.relative
        {
            false
        } else {
            const EPSILON: f32 = 0.00001;
            (actual.velocity - expected.velocity).abs() < EPSILON
                && (actual.acceleration - expected.acceleration).abs() < EPSILON
        }
    }

    fn deserialize_someip_bytes(someip_bytes: &[u8]) -> ObjectSensorData {
        if someip_bytes.len() < 14 {
            panic!(
                "Expected SOME/IP payload to be at least 14 bytes, found length = {}",
                someip_bytes.len()
            );
        }
        let (type_byte, remaining_bytes) = someip_bytes.split_at(1);
        let object_type = match type_byte[0] {
            1 => ObjectType::VEHICLE,
            2 => ObjectType::PEDESTRIAN,
            _ => ObjectType::UNKNOWN,
        }
        .into();

        let (int_bytes, remaining_bytes) = remaining_bytes.split_at(std::mem::size_of::<u32>());
        let distance = u32::from_be_bytes(int_bytes.try_into().unwrap());

        let (float_bytes, remaining_bytes) = remaining_bytes.split_at(std::mem::size_of::<f32>());
        let velocity = f32::from_be_bytes(float_bytes.try_into().unwrap());

        let (float_bytes, remaining_bytes) = remaining_bytes.split_at(std::mem::size_of::<f32>());
        let acceleration = f32::from_be_bytes(float_bytes.try_into().unwrap());

        let relative = remaining_bytes[0] == 1;

        ObjectSensorData {
            object_type,
            distance,
            velocity,
            acceleration,
            relative,
            ..Default::default()
        }
    }

    fn create_reference_message() -> ObjectSensorData {
        ObjectSensorData {
            object_type: ObjectType::VEHICLE.into(),
            distance: 10,
            velocity: 4.2f32,
            acceleration: 1.0f32,
            relative: true,
            ..Default::default()
        }
    }

    fn create_context() -> ContextRef {
        let fqin = ServiceFqin::builder()
            .sdv_vm_name("vm")
            .sdv_package_name("com.android.sdv.sample.someip")
            .service_bundle_name("ByteForwardingConsumer")
            .service_instance_name("instance1")
            .build()
            .expect("Encountered error while building ByteForwardingConsumer Service FQIN");

        ContextRef::create(fqin)
    }
}
