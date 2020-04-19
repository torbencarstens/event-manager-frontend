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

pub static BACKEND_URL: &'static str = "http://localhost:8001/graphql";
// pub static BACKEND_URL: &'static str = "http://events.carstens.tech/graphql";
