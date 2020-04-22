extern crate chrono;
extern crate graphql_client;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use std::env::VarError;

pub use helper::*;
pub use pagination::*;

pub mod helper;
pub mod pagination;

pub fn backend_url() -> String {
    match std::env::var("BACKEND_URL") {
        Ok(var) => {
            var
        }
        Err(_) => {
            if std::env::var("ROCKET_ENV").unwrap_or("dev".to_string()).starts_with("prod") {
                "http://event-manager/graphql"
            } else {
                "http://localhost:8001/graphql"
            }
        }.to_string()
    }
}
