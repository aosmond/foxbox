/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate mio;

use context::{ ContextTrait, SharedContext };
use dummy_adapter::DummyAdapter;
use events::{ EventData, EventSender };
use http_server::HttpServer;
use upnp::UpnpManager;
use mio::EventLoop;
use service::{ Service, ServiceAdapter };
use std::collections::HashMap;

pub struct Controller {
    sender: EventSender,
    context: SharedContext,
    adapters: HashMap<String, Box<ServiceAdapter>>,
    upnp: UpnpManager
}

impl Controller {
    /// Construct a new `Controller`.
    ///
    /// ```
    /// # use service_manager::Controller;
    /// let controller = Controller::new();
    /// ```
    pub fn new(sender: EventSender, context: SharedContext) -> Controller {
        let upnp = UpnpManager::new(sender.clone());
        Controller {
            sender: sender,
            context: context,
            adapters: HashMap::new(),
            upnp: upnp
        }
    }

    pub fn start(&mut self) {
        println!("Starting controller");

        // Start the http server.
        let mut http_server = HttpServer::new(self.context.clone());
        http_server.start();

        // Start the dummy adapter.
        let dummy_adapter = Box::new(DummyAdapter::new(self.sender.clone(), self.context.clone()));
        dummy_adapter.start();
        self.adapters.insert(dummy_adapter.get_name(), dummy_adapter);

        // Start UPnP service discovery
        self.upnp.start().unwrap();
    }
}

impl mio::Handler for Controller {
    type Timeout = ();
    type Message = EventData;

    fn notify(&mut self,
              _: &mut EventLoop<Controller>,
              data: EventData) {
        println!("Receiving a notification! {}", data.description());

        let mut context = self.context.lock().unwrap();
        match data {
            EventData::ServiceStart { id } => {
                // The service should be added already, panic if that's not the
                // case.
                match context.get_service(&id) {
                    None => panic!(format!("Missing service with id {}", id)),
                    Some(_) => {}
                }

                println!("ServiceStart {} We now have {} services.", id, context.services_count());
            }
            EventData::ServiceStop { id } => {
                context.remove_service(id.clone());
                println!("ServiceStop {} We now have {} services.", id, context.services_count());
            }
            EventData::UpnpServiceDiscovered { ref service } => {
                for (name, adapter) in &mut self.adapters {
                    if adapter.upnp_discover(service) {
                        println!("{} claimed upnp service {}", name, service.msearch.device_id);
                        break;
                    }
                }
            }
            EventData::UpnpSearch { target } => {
                let _ = self.upnp.search(target);
            }
            _ => { }
        }
    }
}
