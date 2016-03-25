/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

/// An adapter dedicated to the Philips Hue
pub mod philips_hue;
pub mod new_hue;

/// An adapter providing time services.
pub mod clock;

/// An adapter providing WebPush services.
pub mod webpush;

pub mod ip_camera;

use foxbox_taxonomy::adapter::AdapterManagerHandle;
use foxbox_taxonomy::api::API;

use service::ServiceAdapter;
use traits::Controller;

pub struct AdapterManager<T> {
    controller: T,
    adapters: Vec<Box<ServiceAdapter>>,
}

impl<T: Controller> AdapterManager<T> {
    pub fn new(controller: T) -> Self {
        debug!("Creating Adapter Manager");
        AdapterManager {
            controller: controller,
            adapters: Vec::new(),
        }
    }

    /// Start all the adapters.
    pub fn start<A>(&mut self, adapter_manager: A)
        where A: AdapterManagerHandle + API + Send + Clone + 'static {
        let c = self.controller.clone(); // extracted here to prevent double-borrow of 'self'
        //self.start_adapter(Box::new(philips_hue::PhilipsHueAdapter::new(c.clone())));
        new_hue::PhilipsHue::init(adapter_manager.clone(), c.clone()).unwrap(); // FIXME: We should have a way to report
        clock::Clock::init(&adapter_manager).unwrap(); // FIXME: We should have a way to report errors
        webpush::WebPush::init(c, &adapter_manager).unwrap();
        ip_camera::IPCameraAdapter::init(adapter_manager.clone(), self.controller.clone()).unwrap();
    }

    // fn start_adapter(&mut self, adapter: Box<ServiceAdapter>) {
    //     adapter.start();
    //     self.adapters.push(adapter);
    // }

    /// Stop all the adapters.
    pub fn stop(&self) {
        for adapter in &self.adapters {
            adapter.stop();
        }
    }
}
