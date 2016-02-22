/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use context::{ ContextTrait, SharedContext };
use events::*;
use iron::{ Request, Response, IronResult };
use iron::headers::ContentType;
use iron::status::Status;
use router::Router;
use service::{ Service, ServiceAdapter, ServiceProperties };
use upnp::UpnpService;
use std::time::Duration;
use std::thread;
use uuid::Uuid;

struct DummyService {
    properties: ServiceProperties,
    sender: EventSender,
    dont_kill: bool
}

impl DummyService {
    fn new(sender: EventSender, context: SharedContext, id: u32) -> DummyService {
        println!("Creating dummy service");
        let ctx_clone = context.clone();
        let ctx = ctx_clone.lock().unwrap();
        let service_id = Uuid::new_v4().to_simple_string();
        DummyService {
            properties: ServiceProperties {
                id: service_id.clone(),
                name: "dummy service".to_owned(),
                description: "really nothing to see".to_owned(),
                http_url: ctx.get_http_root_for_service(service_id.clone()),
                ws_url: ctx.get_ws_root_for_service(service_id)
            },
            sender: sender,
            dont_kill: id % 3 == 0
        }
    }
}

impl Service for DummyService {
    fn get_properties(&self) -> ServiceProperties {
        self.properties.clone()
    }

    // Starts the service, it will just spawn a thread and send messages once
    // in a while.
    fn start(&self) {
        let sender = self.sender.clone();
        let props = self.properties.clone();
        let can_kill = !self.dont_kill;
        thread::spawn(move || {
            println!("Hello from dummy service thread!");
            let mut i = 0;
            loop {
                thread::sleep(Duration::from_millis(1000));
                println!("Bip #{} from {}", i, props.id);
                i += 1;
                if i == 3 && can_kill {
                    break;
                }
            }
            sender.send(EventData::ServiceStop { id: props.id.to_string() }).unwrap();
        });
    }

    fn stop(&self) {
        println!("Stopping dummy service");
    }

    // Processes a http request.
    fn process_request(&self, req: &Request) -> IronResult<Response> {
        let cmd = req.extensions.get::<Router>().unwrap().find("command").unwrap_or("");
        let mut response = Response::with(format!("Got command {} at url {}", cmd, req.url));
        response.status = Some(Status::Ok);
        response.headers.set(ContentType::plaintext());
        Ok(response)
    }
}

pub struct DummyAdapter {
    name: String,
    sender: EventSender,
    context: SharedContext,
    rediscover: bool
}

impl DummyAdapter {
    pub fn new(sender: EventSender,
           context: SharedContext) -> DummyAdapter {
        println!("Creating dummy adapter");
        DummyAdapter { name: "DummyAdapter".to_owned(),
                       sender: sender,
                       context: context,
                       rediscover: true
                     }
    }
}

impl ServiceAdapter for DummyAdapter {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn start(&self) {
        let sender = self.sender.clone();
        let mut id = 0;
        let context = self.context.clone();
        thread::spawn(move || {
            sender.send(EventData::AdapterStart { name: "Dummy Service Adapter".to_owned() }).unwrap();
            loop {
                thread::sleep(Duration::from_millis(2000));
                id += 1;
                let service = DummyService::new(sender.clone(), context.clone(), id);
                let service_id = service.get_properties().id;
                service.start();
                let mut ctx = context.lock().unwrap();
                ctx.add_service(Box::new(service));
                sender.send(EventData::ServiceStart { id: service_id }).unwrap();

                // Create at most 7 dummy services.
                if id == 7 {
                    break;
                }
            }
        });
    }

    fn stop(&self) {
        println!("Stopping dummy adapter");
    }

    fn upnp_discover(&mut self, service: &UpnpService) -> bool {
        let desc = &service.description;

        // Let the dummy adapter own the Hue simulator
        let owns = service.msearch.device_id.contains("uuid:2f402f80-da50-11e1-9b23-") &&
                   desc.get("/root/device/modelName").unwrap() == "Philips hue bridge 2012" &&
                   desc.get("/root/device/modelNumber").unwrap() == "929000226503";
        if owns {
            println!("Found Phillips Hue simulator upnp service: {:?}", service);
            if self.rediscover {
                self.rediscover = false;
                let sender = self.sender.clone();
                let target = match desc.get("/root/device/deviceType") {
                    Some(x) => Some(x.to_lowercase()),
                    None => None
                };
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(10000));
                    println!("Requesting rediscovery of Hue simulator");
                    sender.send(EventData::UpnpSearch { target: target }).unwrap();
                });
            }
        }
        owns
    }
}
