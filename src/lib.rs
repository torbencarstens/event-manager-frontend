extern crate chrono;
extern crate graphql_client;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

pub use helper::*;
pub use pagination::*;

pub mod helper;
pub mod pagination;

pub fn backend_url() -> String {
    // std::env::var("BACKEND_URL").unwrap_or("http://localhost:8001/graphql".to_string())
    "http://events.carstens.tech/graphql".to_string()
}
// pub static BACKEND_URL: &'static str = "http://events.carstens.tech/graphql";
