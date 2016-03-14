/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use foxbox_taxonomy::adapter::AdapterManagerHandle;
use foxbox_taxonomy::api::API;
use serde_json;
use service::Service;
use std::collections::{ BTreeMap, HashMap };
use std::sync::{ Arc, Mutex, RwLock };
use std::thread;
use std::time::Duration;
use super::hub_api::HubApi;
use super::light;
use super::structs;
use traits::Controller;
use uuid::Uuid;

pub struct Hub<C> {
    id: String,
    ip: String,
    controller: C,
    api: Arc<HubApi>,
    lights: Arc<RwLock<HashMap<String, Arc<light::Light>>>>,
}

impl<C: Controller> Hub<C> {
    pub fn new(id: &str, ip: &str, controller: C) -> Self {
        // Get API token from config store, default to a random UUID.
        let token = controller.get_config().get_or_set_default(
            "philips_hue",
            &format!("token_{}", id),
            &Uuid::new_v4().to_simple_string());
        Hub {
            id: id.to_owned(),
            ip: ip.to_owned(),
            controller: controller,
            api: Arc::new(HubApi::new(id, ip, &token)),
            lights: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start<A>(&self, adapt: A) where A: AdapterManagerHandle + Send + Clone + 'static {
        let controller = self.controller.clone();
        let api = self.api.clone();
        let adapt = adapt.clone();
        let lights = self.lights.clone();

        thread::spawn(move || {

            // The main Hub management loop
            loop {
                if !api.is_available() {
                    // Re-check availability every minute.
                    thread::sleep(Duration::from_millis(60*1000));
                    continue;
                }

                // If the Hub is not paired, try pairing.
                if !api.is_paired() {
                    info!("Push pairing button on Philips Hue Bridge ID {}", api.id);

                    // Try pairing for 120 seconds.
                    for _ in 0..120 {
                        controller.adapter_notification(
                            json_value!({ adapter: "philips_hue",
                                message: "NeedsPairing", hub: api.id }));
                        if api.try_pairing() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(1000));
                    }

                    if api.is_paired() {
                        info!("Paired with Philips Hue Bridge ID {}", api.id);
                        controller.adapter_notification(
                            json_value!({ adapter: "philips_hue", message: "PairingSuccess",
                                hub: api.id }));
                    } else {
                        warn!("Pairing timeout with Philips Hue Bridge ID {}", api.id);
                        controller.adapter_notification(
                            json_value!({ adapter: "philips_hue", message: "PairingTimeout",
                                hub: api.id }));
                        // Giving up for this Hub.
                        // Re-try pairing every hour.
                        thread::sleep(Duration::from_millis(60*60*1000));
                        continue;
                    }
                }

                // We have a paired Hub, instantiate the lights services.
                // Extract and log some info
                let setting = api.get_settings();
                let hs = structs::Settings::new(&setting).unwrap(); // TODO: no unwrap
                info!(
                    "Connected to Philips Hue bridge model {}, ID {}, software version {}, IP address {}",
                    hs.config.modelid, hs.config.bridgeid, hs.config.swversion,
                    hs.config.ipaddress);

                let light_ids = api.get_lights();
                for light_id in light_ids {
                    debug!("Found light {} on hub {}", light_id, api.id);
                    let new_light = Arc::new(light::Light::new(adapt.clone(), api.clone(), &light_id));
                    new_light.start();
                    lights.write().unwrap().insert(light_id, new_light);
                }

                loop { // forever
                    thread::sleep(Duration::from_millis(60*1000));
                }
            }
        });
    }

    pub fn stop(&self) {
        info!("Stopping Philips Hue Bridge service for ID {}", self.id);
        let lights = self.lights.clone();
        let mut lights_write = lights.write().unwrap();
        for (_, light) in lights_write.drain() {
            light.stop();
        }
    }

    pub fn get_light_status(&self, id: &str) -> structs::SettingsLightEntry {
        let url = format!("lights/{}", id);
        let res = self.api.get(&url).unwrap(); // TODO: remove unwrap
        structs::parse_json(&res).unwrap() // TODO no unwrap
    }

    pub fn set_light_color(&self, light_id: &str, hue: u32, sat: u32, val: u32, on: bool) {
        let url = format!("lights/{}/state", light_id);
        let cmd = json!({ hue: hue, sat: sat, bri: val, on: on });
        let _ = self.api.put(&url, &cmd);
    }

}
