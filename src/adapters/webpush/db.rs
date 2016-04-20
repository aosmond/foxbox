/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Stores subscription information for `WebPush`.
//!
//! # The `WebPush` database
//!
//! The "subscriptions" table stores the `WebPush` subscription metadata
//! necessary to encrypt a message (ECDH public key) and to send said
//! message to a push service (push URI). The user is identified via
//! an ID shared with the Users database. Each user may have any number
//! of active subscriptions.
//!
//! The "resources" table stores the resources that the user is watching
//! in order to receive notifications. Adapters may publish a message
//! to a given resource and all users watching that resource will be
//! issued a push notification on each of their subscriptions.
//!

use super::Subscription;
use libc::c_int;
use rusqlite::{ self, Connection };

fn escape(string: &str) -> String {
    // http://www.sqlite.org/faq.html#q14
    string.replace("'", "''")
}

fn escape_option(opt: &Option<String>) -> Option<String> {
    match *opt {
        Some(ref x) => { return Some(escape(x)); },
        None => { return None; },
    };
}

pub struct WebPushDb {
    db: Connection,
}

impl WebPushDb {
    /// Opens the database at `path` and creates it if not available yet.
    pub fn new(path: &str) -> Self {
        let db = Connection::open(path).unwrap();
        db.execute("CREATE TABLE IF NOT EXISTS subscriptions (
                    user_id     INTEGER,
                    push_uri    TEXT NOT NULL UNIQUE,
                    public_key  TEXT NOT NULL,
                    auth        TEXT
            )", &[]).unwrap();

        WebPushDb {
            db: db
        }
    }

    /// Adds a new push subscription `sub` bound to the user `user_id`.
    pub fn subscribe(&self, user_id: i32, sub: &Subscription) -> rusqlite::Result<c_int> {
        self.db.execute("INSERT INTO subscriptions VALUES ($1, $2, $3, $4)",
                        &[&user_id, &escape(&sub.push_uri), &escape(&sub.public_key), &escape_option(&sub.auth)]
        )
    }

    /// Removes an existing push subscription identified by `push_uri`.
    pub fn unsubscribe(&self, _: i32, push_uri: &str) -> rusqlite::Result<c_int> {
        self.db.execute("DELETE FROM subscriptions WHERE push_uri=$1",
                        &[&escape(push_uri)]
        )
    }

    /// Gets the push subscriptions for the user `user_id`.
    pub fn get_subscriptions(&self, user_id: Option<i32>) -> rusqlite::Result<Vec<Subscription>> {
        let mut subs = Vec::new();
        let mut stmt;
        let rows = match user_id {
            Some(uid) => {
                stmt = try!(self.db.prepare("SELECT push_uri, public_key, auth FROM subscriptions WHERE user_id=$1"));
                try!(stmt.query(&[&uid]))
            },
            None => {
                stmt = try!(self.db.prepare("SELECT push_uri, public_key, auth FROM subscriptions"));
                try!(stmt.query(&[]))
            }
        };
        let (count, _) = rows.size_hint();
        subs.reserve_exact(count);
        for result_row in rows {
            let row = try!(result_row);
            subs.push(Subscription {
                push_uri: row.get(0),
                public_key: row.get(1),
                auth: row.get(2)
            });
        }
        Ok(subs)
    }
}

#[cfg(test)]
pub fn get_db_environment() -> String {
    use libc::getpid;
    use std::thread;
    let tid = format!("{:?}", thread::current());
    format!("./webpush_db_test-{}-{}.sqlite", unsafe { getpid() }, tid.replace("/", "42"))
}

#[cfg(test)]
pub fn remove_test_db() {
    use std::path::Path;
    use std::fs;

    let dbfile = get_db_environment();
    match fs::remove_file(Path::new(&dbfile)) {
        Err(e) => panic!("Error {} cleaning up {}", e, dbfile),
        _ => assert!(true)
    }
}

#[cfg(test)]
describe! tests {
    before_each {
        let db = WebPushDb::new(&get_db_environment());
    }

    it "should manage subscription correctly for user" {
        use super::super::Subscription;

        let subs0 = db.get_subscriptions(Some(1)).unwrap();
        assert_eq!(subs0.len(), 0);

        let sub = Subscription {
            push_uri: "test_push_uri".to_owned(),
            public_key: "test_public_key".to_owned(),
            auth: Some("test_auth".to_owned())
        };
        db.subscribe(1, &sub).unwrap();

        let subs1 = db.get_subscriptions(Some(1)).unwrap();
        assert_eq!(subs1.len(), 1);
        assert_eq!(subs1[0], sub);

        db.unsubscribe(1, &sub.push_uri).unwrap();

        let subs2 = db.get_subscriptions(Some(1)).unwrap();
        assert_eq!(subs2.len(), 0);
    }

    it "should yield correct number subscriptions with multiple users" {
        use super::super::Subscription;

        db.subscribe(1, &Subscription {
            push_uri: "u1_sub0_puri".to_owned(),
            public_key: "u1_sub0_pkey".to_owned(),
            auth: Some("u1_sub0_auth".to_owned())
        }).unwrap();
        db.subscribe(1, &Subscription {
            push_uri: "u1_sub1_puri".to_owned(),
            public_key: "u1_sub1_pkey".to_owned(),
            auth: None
        }).unwrap();
        db.subscribe(2, &Subscription {
            push_uri: "u2_sub0_puri".to_owned(),
            public_key: "u2_sub0_pkey".to_owned(),
            auth: Some("u2_sub0_auth".to_owned())
        }).unwrap();
        db.subscribe(3, &Subscription {
            push_uri: "u3_sub0_puri".to_owned(),
            public_key: "u3_sub0_pkey".to_owned(),
            auth: Some("u3_sub0_auth".to_owned())
        }).unwrap();

        let subs1 = db.get_subscriptions(Some(1)).unwrap();
        assert_eq!(subs1.len(), 2);

        let subs2 = db.get_subscriptions(Some(2)).unwrap();
        assert_eq!(subs2.len(), 1);

        let subs3 = db.get_subscriptions(Some(3)).unwrap();
        assert_eq!(subs3.len(), 1);

        let subs_all = db.get_subscriptions(None).unwrap();
        assert_eq!(subs_all.len(), 4);
    }

    after_each {
        remove_test_db();
    }
}
