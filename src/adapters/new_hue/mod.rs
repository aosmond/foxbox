/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

pub mod discovery;
pub mod http;
pub mod hub;
pub mod hub_api;
pub mod light;
pub mod structs;

use foxbox_taxonomy::adapter::*;
use foxbox_taxonomy::api::{ Error, InternalError };
use foxbox_taxonomy::values::{ Duration as ValDuration, Range, TimeStamp, Type, Value };
use foxbox_taxonomy::services::*;

use chrono;
use std::collections::{ HashMap, HashSet };
use std::sync::Arc;
use timer;
use transformable_channels::mpsc::*;
use traits::Controller;

static ADAPTER_NAME: &'static str = "Philips Hue adapter (built-in)";
static ADAPTER_VENDOR: &'static str = "team@link.mozilla.org";
static ADAPTER_VERSION: [u32;4] = [0, 0, 0, 0];

pub struct PhilipsHue<C> {
    controller: C,
    discovery: Arc<discovery::Discovery<C>>,
    /// Timer used to dispatch `register_watch` requests.
    timer: timer::Timer,
    getter_timestamp_id: Id<Getter>,
    getter_time_of_day_id: Id<Getter>,
}

impl<C: Controller> PhilipsHue<C> {
    pub fn init<A>(adapt: A, controller: C) -> Result<(), Error>
        where A: AdapterManagerHandle + Send + Clone + 'static
    {
        let discovery = discovery::Discovery::new(controller.clone());
        let hue_adapter = Box::new(PhilipsHue {
            controller: controller,
            discovery: Arc::new(discovery),
            timer: timer::Timer::new(),
            getter_timestamp_id: Id::new("getter:timestamp.philips_hue@link.mozilla.org"),
            getter_time_of_day_id: Id::new("getter:time_of_day.philips_hue@link.mozilla.org"),
        });
        hue_adapter.discovery.start(adapt.clone());
        try!(adapt.add_adapter(hue_adapter));
        Ok(())
    }
}

impl<C: Controller> PhilipsHue<C> {
    pub fn id() -> Id<AdapterId> {
        Id::new("philips_hue@link.mozilla.org")
    }

    fn create_service_id(service_id: &str) -> Id<ServiceId> {
        Id::new(&format!("service:{}@link.mozilla.org", service_id))
    }

    pub fn create_setter_id(operation: &str, service_id: &str) -> Id<Setter> {
        Self::create_io_mechanism_id("setter", operation, service_id)
    }

    pub fn create_getter_id(operation: &str, service_id: &str) -> Id<Getter> {
        Self::create_io_mechanism_id("getter", operation, service_id)
    }

    fn create_io_mechanism_id<IO>(prefix: &str, operation: &str, service_id: &str) -> Id<IO>
        where IO: IOMechanism
    {
        Id::new(&format!("{}:{}.{}@link.mozilla.org", prefix, operation, service_id))
    }
}

impl<C: Controller> Adapter for PhilipsHue<C> {
    fn id(&self) -> Id<AdapterId> {
        Self::id()
    }

    fn name(&self) -> &str {
        ADAPTER_NAME
    }

    fn vendor(&self) -> &str {
        ADAPTER_VENDOR
    }

    fn version(&self) -> &[u32;4] {
        &ADAPTER_VERSION
    }

    fn fetch_values(&self, mut set: Vec<Id<Getter>>) -> ResultMap<Id<Getter>, Option<Value>, Error> {
        set.drain(..).map(|id| {
            if id == self.getter_timestamp_id {
                let date = TimeStamp::from_datetime(chrono::UTC::now());
                (id, Ok(Some(Value::TimeStamp(date))))
            } else {
                (id.clone(), Err(Error::InternalError(InternalError::NoSuchGetter(id))))
            }
        }).collect()
    }

    fn send_values(&self, mut values: HashMap<Id<Setter>, Value>) -> ResultMap<Id<Setter>, (), Error> {
        values.drain()
            .map(|(id, _)| {
                (id.clone(), Err(Error::InternalError(InternalError::NoSuchSetter(id))))
            })
            .collect()
    }

    fn register_watch(&self, mut watch: Vec<(Id<Getter>, Option<Range>)>,
        _: Box<ExtSender<WatchEvent>>) ->
            ResultMap<Id<Getter>, Box<AdapterWatchGuard>, Error>
    {
        watch.drain(..).map(|(id, filter)| {
            (id.clone(), Err(Error::GetterDoesNotSupportWatching(id.clone())))
        }).collect()
    }
}
