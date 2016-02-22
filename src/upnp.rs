extern crate libc;
extern crate hyper;

use std::collections::HashMap;
use std::ffi::{ CString, CStr };
use std::io::{ Read, Cursor };
use std::ptr;
use std::thread;
use events::*;
use util::parse_simple_xml;

#[allow(dead_code)]
#[derive(Debug)]
#[repr(C)]
enum EventType {
    ControlActionRequest,
    ControlActionComplete,
    ControlGetVarRequest,
    ControlGetVarComplete,
    DiscoveryAdvertisementAlive,
    DiscoveryAdvertisementByebye,
    DiscoverySearchResult,
    DiscoverySearchTimeout,
    SubscriptionRequest,
    Received,
    RenewalComplete,
    SubscribeComplete,
    UnsubscribeComplete,
    AutorenewalFailed,
    SubscriptionExpired
}

const LINE_SIZE: usize = 180;

#[repr(C)]
struct Discovery {
    err_code: libc::c_int,
    expires: libc::c_int,
    device_id: [libc::c_char; LINE_SIZE],
    device_type: [libc::c_char; LINE_SIZE],
    service_type: [libc::c_char; LINE_SIZE],
    service_ver: [libc::c_char; LINE_SIZE],
    location: [libc::c_char; LINE_SIZE],
    os: [libc::c_char; LINE_SIZE],
    date: [libc::c_char; LINE_SIZE],
    ext: [libc::c_char; LINE_SIZE],
    dest_addr: *mut libc::sockaddr_in,
}

type ClientHandle = libc::c_int;

type ClientCallbackPtr = extern fn(event_type: EventType, event: *const libc::c_void, cookie: *mut libc::c_void);

#[link(name = "upnp")]
extern {
    fn UpnpInit(hostIp: *const libc::c_char, destPort: libc::c_ushort) -> libc::c_int;
    fn UpnpRegisterClient(callback: ClientCallbackPtr, cookie: *mut libc::c_void, handle: *mut ClientHandle) -> libc::c_int;
    fn UpnpUnRegisterClient(handle: ClientHandle) -> libc::c_int;
    fn UpnpSearchAsync(handle: ClientHandle, maxAttempts: libc::c_int, target: *const libc::c_char, cookie: *const libc::c_void) -> libc::c_int;
}

#[derive(Debug)]
pub struct UpnpMsearchHeader {
    pub device_id: String,
    pub device_type: String,
    pub service_type: String,
    pub service_ver: String,
    pub location: String,
    pub os: String,
    pub date: String,
    pub ext: String,
    pub expires: i32,
    pub alive: bool,
}

#[derive(Debug)]
pub struct UpnpService {
    pub msearch: UpnpMsearchHeader,
    pub description: HashMap<String, String>,
    pub description_data: String
}

pub struct UpnpManager {
    sender: EventSender,
    handle: ClientHandle,
    cookie: *mut EventSender
}

impl Drop for UpnpManager {
    fn drop(&mut self) {
        println!("Upnp: Releasing manager (handle={} cookie={:?})", self.handle, self.cookie);
        unsafe {
            UpnpUnRegisterClient(self.handle);
            Box::from_raw(self.cookie);
        }
    }
}

impl UpnpManager {
    pub fn new(sender: EventSender) -> UpnpManager {
        UpnpManager {
            sender: sender,
            handle: 0,
            cookie: ptr::null_mut()
        }
    }

    fn msearch_callback(sender: EventSender, data: &Discovery, alive: bool) {
        let header = UpnpMsearchHeader {
            device_id: unsafe { CStr::from_ptr(&data.device_id[0]).to_string_lossy().into_owned() },
            device_type: unsafe { CStr::from_ptr(&data.device_type[0]).to_string_lossy().into_owned() },
            service_type: unsafe { CStr::from_ptr(&data.service_type[0]).to_string_lossy().into_owned() },
            service_ver: unsafe { CStr::from_ptr(&data.service_ver[0]).to_string_lossy().into_owned() },
            location: unsafe { CStr::from_ptr(&data.location[0]).to_string_lossy().into_owned() },
            os: unsafe { CStr::from_ptr(&data.os[0]).to_string_lossy().into_owned() },
            date: unsafe { CStr::from_ptr(&data.date[0]).to_string_lossy().into_owned() },
            ext: unsafe { CStr::from_ptr(&data.ext[0]).to_string_lossy().into_owned() },
            expires: data.expires,
            alive: alive,
        };

        // No need to fetch the description XML if the device notified us
        // that it is disconnecting; should be even bother to tell adapters
        // about this?
        if !alive {
            let service = UpnpService {
                msearch: header,
                description: HashMap::new(),
                description_data: String::new()
            };
            sender.send(EventData::UpnpServiceDiscovered { service: service }).unwrap();
            return;
        }

        thread::spawn(move || {
            // Note we must be careful to actually handle these errors gracefully
            // since the network or end device can fail us easily.
            let client = hyper::Client::new();
            let mut res = match client.get(&header.location).header(hyper::header::Connection::close()).send() {
                Ok(x) => x,
                Err(e) => { println!("Upnp: failed to send request {}: {:?}", header.location, e); return; }
            };

            let mut body = String::new();
            match res.read_to_string(&mut body) {
                Ok(x) => x,
                Err(e) => { println!("Upnp: failed to get response {}: {:?}", header.location, e); return; }
            };

            let values;
            {
                let cursor = Cursor::new(&body);
                values = match parse_simple_xml(cursor) {
                    Ok(x) => x,
                    Err(e) => { println!("Upnp: failed to parse response {}: {:?}", header.location, e); return; }
                };
            }

            let service = UpnpService {
                msearch: header,
                description: values,
                description_data: body
            };
            sender.send(EventData::UpnpServiceDiscovered { service: service }).unwrap();
        });
    }

    extern fn callback(event_type: EventType, event: *const libc::c_void, cookie: *mut libc::c_void) {
        let this: *mut EventSender = cookie as *mut EventSender;
        if this == ptr::null_mut() { panic!("invalid cookie"); }

        let data: *const Discovery;
        let alive: bool;
        match event_type {
            EventType::DiscoverySearchResult | EventType::DiscoveryAdvertisementAlive => {
                data = event as *const Discovery;
                alive = true;
            },
            EventType::DiscoveryAdvertisementByebye => {
                data = event as *const Discovery;
                alive = false;
            },
            // Timeout really just lets us know the search is done, it may or may not
            // have found devices
            EventType::DiscoverySearchTimeout => { return; },
            _ => { println!("Upnp: unhandled callback event {:?}", event_type); return; },
        };

        if data == ptr::null() { panic!("null discovery"); }
        unsafe { UpnpManager::msearch_callback((*this).clone(), &(*data), alive); }
    }

    fn initialize() -> Result<(), i32> {
        let err = unsafe { UpnpInit(ptr::null(), 0) };
        println!("Upnp: initialized ({})", err);
        match err {
            0 => Ok(()),
            _ => Err(err)
        }
    }

    pub fn search(&self, target: Option<String>) -> Result<(), i32> {
        let target = match target {
            Some(x) => CString::new(x),
            None => CString::new("ssdp:all")
        }.unwrap();

        let cookie = self.cookie as *mut libc::c_void;
        let err = unsafe { UpnpSearchAsync(self.handle, 1, target.as_ptr(), cookie) };

        println!("Upnp: search for devices matching {:?} ({})", target, err);
        match err {
            0 => Ok(()),
            _ => Err(err)
        }
    }

    pub fn start(&mut self) -> Result<(), i32> {
        UpnpManager::initialize().unwrap();

        self.cookie = Box::into_raw(Box::new(self.sender.clone()));
        let cookie = self.cookie as *mut libc::c_void;
        let handle: *mut ClientHandle = &mut self.handle as *mut ClientHandle;
        let err = unsafe { UpnpRegisterClient(UpnpManager::callback, cookie, handle) };

        println!("Upnp: registered client ({})", err);
        match err {
            0 => { },
            _ => return Err(err)
        };

        self.search(None)
    }
}
