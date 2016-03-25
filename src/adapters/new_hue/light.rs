/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use foxbox_taxonomy::adapter::*;
use foxbox_taxonomy::api::{ Error, InternalError };
use foxbox_taxonomy::services::*;
use std::collections::HashSet;
use std::sync::Arc;
use super::hub_api::HubApi;

const CUSTOM_PROPERTY_MANUFACTURER: &'static str = "manufacturer";
const CUSTOM_PROPERTY_MODEL: &'static str = "model";
const CUSTOM_PROPERTY_NAME: &'static str = "name";

pub struct Light<A>
    where A: AdapterManagerHandle + Send + Clone + 'static {
    id: String,
    api: Arc<HubApi>,
    adapt: A,
}

impl<A> Light<A>
    where A: AdapterManagerHandle + Send + Clone + 'static
{
    pub fn new(adapt: A, api: Arc<HubApi>, light_id: &str) -> Self
    {
        debug!("Creating ColorLight with ID {} on hub {}", light_id, api.id);
        Light{
            id: light_id.to_owned(),
            api: api,
            adapt: adapt,
        }
    }

    pub fn start(&self) {
        let status = self.api.get_light_status(&self.id);
        match status.lighttype.as_ref() {
            "Extended color light" => {
                info!("New Extended Color Light {} on Bridge {}",
                    self.id, self.api.id);
                let light_id = status.uniqueid;
                // let service_id = self.adapt.get_service_id(light_id);
                // let mut service = Service::empty(service_id, self.adapt.id());
                // service.properties.insert(CUSTOM_PROPERTY_MANUFACTURER.to_owned(),
                //     status.manufacturername.to_owned());
                // service.properties.insert(CUSTOM_PROPERTY_MODEL.to_owned(),
                //     status.modelid.to_owned());
                // service.properties.insert(CUSTOM_PROPERTY_NAME.to_owned(),
                //     status.name.to_owned());
                // try!(self.adapt.add_service(service));
                //
                // let getter_id = self.adapt.create_getter_id("power_getter", light_id);
                // try!(self.adapt.add_getter(Channel {
                //         tags: HashSet::new(),
                //         adapter: self.adapt.id(),
                //         id: getter_id.clone(),
                //         last_seen: None,
                //         service: service_id.clone(),
                //         mechanism: Getter {
                //             kind: ChannelKind::OnOff,
                //             updated: None
                //         }
                // }));
            },
            _ => {
                warn!("Hue Light {} on Bridge {} has unsupported type `{}`",
                    self.id, self.api.id, status.lighttype);
            }
        }
    }

    pub fn stop(&self) {
        unimplemented!();
    }
}
