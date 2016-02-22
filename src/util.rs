use std::collections::HashMap;
use std::io::Read;
use xml::reader::{ EventReader, XmlEvent };

pub fn parse_simple_xml<R: Read>(data: R) -> Result<HashMap<String, String>, String> {
    let parser = EventReader::new(data);
    let mut values = HashMap::<String, String>::new();
    let mut ignore = HashMap::<String, bool>::new();
    let mut key = String::new();
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, .. }) => {
                key.push('/');
                key.push_str(name.local_name.as_str());
                if !ignore.contains_key(&key) && values.contains_key(&key) {
                    // Array of elements, let's just ignore those for now and maybe
                    // find a better XML parser ;)
                    ignore.insert(key.clone(), true);
                    values.remove(&key);
                }
            }
            Ok(XmlEvent::EndElement { name, .. }) => {
                // Should ensure truncated name and given name match?
                match key.rfind('/') {
                    Some(x) => { key.truncate(x); }
                    _ => { return Err(format!("broken key {} at ending element {}", key, name)); }
                }
            }
            Ok(XmlEvent::Characters(x)) | Ok(XmlEvent::CData(x)) => {
                if !ignore.contains_key(&key) {
                    values.entry(key.clone()).or_insert_with(|| { String::new() }).push_str(x.as_str());
                }
            }
            Err(e) => { return Err(format!("parse error {}", e)); }
            _ => { }
        }
    }
    Ok(values)
}
