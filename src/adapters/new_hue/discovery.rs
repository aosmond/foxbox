/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use foxbox_taxonomy::adapter::AdapterManagerHandle;
use serde_json;
use std::collections::HashMap;
use std::sync::{ Arc, RwLock };
use std::thread;
use std::time::Duration;
use super::http;
use super::hub;
use traits::Controller;

pub struct Discovery<C> {
    controller: C,
    hubs: Arc<RwLock<HashMap<String, hub::Hub<C>>>>,
}

impl<C: Controller> Discovery<C> {
    pub fn new(controller: C) -> Self {
        Discovery {
            controller: controller,
            hubs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start<A>(&self, adapt: A)
        where A: AdapterManagerHandle + Send + Clone + 'static
    {
        let controller = self.controller.clone();
        let hubs = self.hubs.clone();
        let adapt = adapt.clone();

        thread::spawn(move || {
            let nupnp_url = controller.get_config().get_or_set_default(
                    "philips_hue", "nupnp_url", "http://www.meethue.com/api/nupnp");
            let nupnp_hubs = nupnp_query(&nupnp_url);
            debug!("nUPnP reported Philips Hue bridges: {:?}", nupnp_hubs);

            for nupnp in nupnp_hubs {
                let hub = hub::Hub::new(&nupnp.id, &nupnp.internalipaddress, controller.clone());
                hub.start(adapt.clone());
                // This will lead to weirdness if hubs with identical IDs
                // are popping up on the network. (Think DoS!)
                hubs.write().unwrap().insert(nupnp.id, hub);
            }

            loop { // Forever
                thread::sleep(Duration::from_millis(60*1000));
            }
        });
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NupnpEntry {
    id: String,
    internalipaddress: String
}

pub fn nupnp_query(server_url: &str) -> Vec<NupnpEntry> {
    // "[{\"id\":\"001788fffe243755\",\"internalipaddress\":\"192.168.5.129\"}]"
    debug!("Querying NUPnP server at {}", server_url);
    let empty_list = Vec::new();
    let nupnp_list = http::get(server_url)
        .map(parse_nupnp_response)
        .unwrap_or(empty_list);
    // let nupnp_list = parse_nupnp_response(r#"[{ "id": "001788fffe25681a", "internalipaddress": "192.168.2.4" }]"#.to_owned());
    debug!("Parsed NUPnP response: {:?}", nupnp_list);
    nupnp_list
}

fn parse_nupnp_response(content: String) -> Vec<NupnpEntry> {
    match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(error) => {
            warn!("Unable to parse NUPnP response: {}", error.to_string());
            Vec::<NupnpEntry>::new()
        }
    }
}
