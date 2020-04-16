#![feature(decl_macro, proc_macro_hygiene)]

extern crate chrono;
extern crate events_frontend;
extern crate graphql_client;
extern crate ics;
extern crate reqwest;
#[macro_use]
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate uuid;

use std::cmp::max;
use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use std::net::IpAddr;

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use graphql_client::{GraphQLQuery, Response};
use ics::properties::{Class, Created, Description, DtEnd, DtStart, Status, Summary, URL};
use rocket::{Request, request};
use rocket::request::{FlashMessage, FromRequest, Outcome};
use rocket::response::{Content, Flash, Redirect, Stream};
use rocket_contrib::serve::StaticFiles;
use rocket_contrib::templates::handlebars::{Context, Handlebars, Helper, HelperResult, JsonRender, JsonValue, Output, Renderable, RenderContext, RenderError};
use rocket_contrib::templates::handlebars::template::{HelperTemplate, Parameter, TemplateElement};
use rocket_contrib::templates::Template;

use events_frontend::{BACKEND_URL, PaginationContext};
use events_frontend::helper::*;

#[derive(Clone, Debug, Deserialize, GraphQLQuery, Serialize)]
#[graphql(
schema_path = "resources/schema.graphql",
query_path = "resources/locations.graphql",
response_derives = "Deserialize, Serialize, Debug"
)]
struct Location {
    id: i64,
    name: String,
    website: Option<String>,
    street: String,
    street_number: i32,
    city: String,
    postal_code: i32,
    country: String,
    building: Option<String>,
    maps_link: String,
}

impl Location {
    fn to_ics(&self) -> ics::properties::Location {
        ics::properties::Location::new(
            format!("{} {}, {} {}", self.street, self.street_number, self.postal_code, self.city)
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Organizer {
    id: i64,
    name: String,
    website: Option<String>,
}

impl From<location::LocationLocation> for Location {
    fn from(input: location::LocationLocation) -> Location {
        Location {
            id: input.id,
            name: input.name,
            website: input.website,
            street: input.street,
            street_number: input.street_number as i32,
            city: input.city,
            postal_code: input.postal_code as i32,
            country: input.country,
            building: input.building,
            maps_link: input.maps_link,
        }
    }
}

impl From<event::EventEvent> for Event {
    fn from(event: event::EventEvent) -> Event {
        Event {
            id: event.id,
            name: event.name,
            description: event.description,
            time: NaiveDateTime::from_timestamp(event.timestamp.parse::<i64>().unwrap(), 0),
            time_end: NaiveDateTime::from_timestamp(event.timestamp_end.parse::<i64>().unwrap(), 0),
            price: event.price,
            currency: event.currency,
            location: Location {
                id: event.location.id,
                name: event.location.name,
                website: event.location.website,
                street: event.location.street,
                street_number: event.location.street_number as i32,
                city: event.location.city,
                postal_code: event.location.postal_code as i32,
                country: event.location.country,
                building: event.location.building,
                maps_link: event.location.maps_link,
            },
            organizer: event.organizer.and_then(|organizer| Some(Organizer {
                id: organizer.id,
                name: organizer.name,
                website: organizer.website,
            })),
        }
    }
}

struct LocationInput {
    id: Option<i64>,
    name: Option<String>,
    website: Option<String>,
    street: Option<String>,
    street_number: Option<i32>,
    city: Option<String>,
    postal_code: Option<i32>,
    country: Option<String>,
    building: Option<String>,
    maps_link: Option<String>,
}

struct EventInput {
    id: Option<i64>,
    name: Option<String>,
    description: Option<String>,
    time: Option<NaiveDateTime>,
    time_end: Option<NaiveDateTime>,
    price: Option<i64>,
    currency: Option<String>,
    location_id: Option<i64>,
    organizer_id: Option<i64>,
}

impl Into<event::Variables> for EventInput {
    fn into(self) -> event::Variables {
        let default_pagination = PaginationContext::default();
        event::Variables {
            constraints: Some(event::Constraints {
                limit: default_pagination.limit.to_string(),
                offset: default_pagination.offset.to_string(),
            }),
            input: Some(event::EventQuery {
                id: self.id,
                name: self.name,
                description: self.description,
                timestamp: self.time.and_then(|t| Some(t.timestamp().to_string())),
                timestamp_end: self.time_end.and_then(|t| Some(t.timestamp().to_string())),
                price: self.price,
                currency: self.currency,
                location_id: self.location_id,
                organizer_id: self.organizer_id,
            }),
        }
    }
}

impl Into<location::Variables> for LocationInput {
    fn into(self) -> location::Variables {
        let default_pagination = PaginationContext::default();

        location::Variables {
            constraints: Some(location::Constraints {
                limit: default_pagination.limit.to_string(),
                offset: default_pagination.offset.to_string(),
            }),
            input: Some(location::LocationQuery {
                id: self.id,
                name: None,
                website: None,
                street: None,
                street_number: None,
                city: None,
                postal_code: None,
                country: None,
                building: None,
                maps_link: None,
            }),
        }
    }
}

#[derive(Clone, Debug, GraphQLQuery, Serialize, Deserialize)]
#[graphql(
schema_path = "resources/schema.graphql",
query_path = "resources/events.graphql",
response_derives = "Deserialize, Serialize, Debug"
)]
struct Event {
    id: i64,
    name: String,
    description: String,
    time: NaiveDateTime,
    time_end: NaiveDateTime,
    price: Option<i64>,
    currency: Option<String>,
    location: Location,
    organizer: Option<Organizer>,
}

// TODO: return an event without a location
fn get_events_for_location(location_id: i64) -> io::Result<Vec<Event>> {
    get_events(EventInput {
        id: None,
        name: None,
        description: None,
        time: None,
        time_end: None,
        price: None,
        currency: None,
        location_id: Some(location_id),
        organizer_id: None,
    }.into())
}

fn get_location(id: i64) -> io::Result<Location> {
    get_locations(LocationInput {
        id: Some(id),
        name: None,
        website: None,
        street: None,
        street_number: None,
        city: None,
        postal_code: None,
        country: None,
        building: None,
        maps_link: None,
    }.into())?
        .pop()
        .ok_or(io::Error::from(io::ErrorKind::NotFound))
}

fn get_locations(variables: location::Variables) -> io::Result<Vec<Location>> {
    let ioerror = |desc| io::Error::new(io::ErrorKind::Other, desc);
    let body = Location::build_query(variables);

    let client = reqwest::blocking::Client::new();
    let res = match client.post(BACKEND_URL).json(&body).send() {
        Ok(val) => Ok(val),
        Err(e) => Err(ioerror(format!("{:#?}", e)))
    }?;
    let response: Response<location::ResponseData> = res.json().map_err(|e|
        ioerror(format!("Couldn't get successful response from server: {}", e))
    )?;
    let data = response.data.ok_or(
        ioerror(format!("Couldn't get data field from response: {:?}", response.errors.and_then(|x| Some(x.into_iter().map(|x| x.message).collect::<Vec<String>>().join(" | ")))))
    )?;

    Ok(data
        .location
        .into_iter()
        .map(From::from)
        .collect::<Vec<Location>>())
}

fn get_events(variables: event::Variables) -> io::Result<Vec<Event>> {
    let ioerror = |desc| io::Error::new(io::ErrorKind::Other, desc);
    let body = Event::build_query(variables);

    let client = reqwest::blocking::Client::new();
    let res = match client.post("http://localhost:8001/graphql").json(&body).send() {
        Ok(val) => Ok(val),
        Err(e) => Err(ioerror(format!("{:#?}", e)))
    }?;
    let response: Response<event::ResponseData> = res.json().map_err(|e|
        ioerror(format!("Couldn't get successful response from server: {}", e))
    )?;
    let data = response.data.ok_or(
        ioerror(format!("Couldn't get data field from response: {:?}", response.errors.and_then(|x| Some(x.into_iter().map(|x| x.message).collect::<Vec<String>>().join(" | ")))))
    )?;

    Ok(data
        .event
        .into_iter()
        .map(From::from)
        .collect::<Vec<Event>>())
}

impl Event {
    fn get_ics(&self) -> ics::ICalendar {
        let mut calendar = ics::ICalendar::new("2.0", "ics-rs");
        let calendar_uuid = uuid::Uuid::new_v4().to_string();
        let dtstamp = self.time.format("%Y%m%dT%H%M%S").to_string();
        let dtendstamp = self.time_end.format("%Y%m%dT%H%M%S").to_string();
        let mut event = ics::Event::new(calendar_uuid, dtstamp.clone());
        event.push(Created::new(dtstamp.clone()));
        event.push(DtStart::new(dtstamp.clone()));
        event.push(DtEnd::new(dtendstamp));
        event.push(Summary::new(&self.name));
        event.push(Description::new(&self.description));
        event.push(self.location.to_ics());
        event.push(Status::needs_action());
        event.push(Class::public());
        event.push(URL::new(format!("https://192.168.178.51:8000/event/{}", self.id))); // TODO: update base url

        calendar.add_event(event);
        calendar
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct DayEventContext {
    day: NaiveDateTime,
    events: Vec<Event>,
}

#[derive(Deserialize, Serialize)]
struct TemplateContext<'a> {
    title: String,
    parent: &'a str,
    page_id: u32,
    days: Vec<DayEventContext>,
    flash: Option<String>,
}


fn get_event(id: i64) -> io::Result<Event> {
    get_events(EventInput {
        id: Some(id),
        name: None,
        description: None,
        time: None,
        time_end: None,
        price: None,
        currency: None,
        location_id: None,
        organizer_id: None,
    }.into())?
        .pop()
        .ok_or(io::Error::from(io::ErrorKind::NotFound))
}

fn get_events_day_contexts(pagination: PaginationContext) -> io::Result<Vec<DayEventContext>> {
    let events = get_events(event::Variables {
        constraints: Some(event::Constraints {
            offset: pagination.offset.to_string(),
            limit: pagination.limit.to_string(),
        }),
        input: None,
    })?;
    let mut map: HashMap<i64, Vec<Event>> = HashMap::new();

    events
        .into_iter()
        .for_each(|event| {
            let date = NaiveDate::from_ymd(event.time.year(), event.time.month(), event.time.day());
            let time = NaiveTime::from_hms(0, 0, 0);
            let start_of_day = NaiveDateTime::new(date, time);
            let day_timestamp = start_of_day.timestamp();

            match map.get_mut(&day_timestamp) {
                Some(day_events) => {
                    day_events.push(event);
                }
                None => {
                    map.insert(day_timestamp, vec![event]);
                }
            }
        });

    let mut result = map
        .into_iter()
        .map(|entry| DayEventContext {
            day: NaiveDateTime::from_timestamp(entry.0, 0),
            events: entry.1,
        })
        .collect::<Vec<DayEventContext>>();
    result.sort_by(|a: &DayEventContext, b: &DayEventContext| a.day.timestamp().cmp(&b.day.timestamp()));

    Ok(result)
}

#[get("/")]
fn index(flash: Option<FlashMessage<'_, '_>>) -> Redirect {
    Redirect::to("/1")
}

#[get("/<id>")]
fn numbered_index(id: Option<u32>, flash: Option<FlashMessage<'_, '_>>) -> Template {
    let page_id = max(1, id.unwrap_or(1));
    let pagination = if page_id > 1 {
        let mut context = PaginationContext::default();
        context.offset = context.limit * (page_id - 1);
        context
    } else {
        PaginationContext::default()
    };
    let days = get_events_day_contexts(pagination).unwrap(); // TODO

    let context = TemplateContext {
        title: "Events".to_string(),
        parent: "layout",
        page_id,
        days,
        flash: flash.and_then(|f| Some(f.msg().to_string())),
    };

    Template::render("index", context)
}

#[derive(Debug, Deserialize, Serialize)]
struct EventTemplateContext<'a> {
    title: String,
    parent: &'a str,
    event: Event,
}

#[derive(Debug, Deserialize, Serialize)]
struct LocationTemplateContext<'a> {
    title: String,
    parent: &'a str,
    location: Location,
    events: Vec<Event>,
}

#[get("/event/<id>")]
fn event(id: i64) -> Template {
    let event = get_event(id).unwrap(); // TODO
    let context = EventTemplateContext {
        title: event.name.clone(),
        parent: "layout",
        event,
    };

    Template::render("event", context)
}

#[get("/event/<id>/ics")]
fn event_ics(id: i64) -> io::Result<Content<Stream<Cursor<Vec<u8>>>>> {
    let mut buffer = Vec::new();
    get_event(id)?.get_ics().write(&mut buffer)?;
    let cursor = Cursor::new(buffer);

    Ok(Content(rocket::http::ContentType::Calendar, Stream::from(cursor)))
}

#[get("/event/<id>/location")]
fn event_location(id: i64) -> io::Result<Redirect> {
    Ok(Redirect::permanent(format!("/location/{}", get_event(id)?.location.id)))
}

#[get("/location/<id>")]
fn location(id: i64) -> Template {
    let location = get_location(id).unwrap(); // TODO
    let events = get_events_for_location(location.id).unwrap();

    let context = LocationTemplateContext {
        title: location.name.clone(),
        parent: "layout",
        location,
        events,
    };

    Template::render("location", context)
}

struct Session {
    test: String
}

impl<'a, 'r> FromRequest<'a, 'r> for Session {
    type Error = std::convert::Infallible;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Session, Self::Error> {
        request::Outcome::Success(Session {
            test: request.client_ip().unwrap_or(IpAddr::from([0, 0, 0, 0])).to_string()
        })
    }
}

#[get("/session")]
fn session(context: Session) -> Flash<Redirect> {
    Flash::success(
        Redirect::to("/"),
        context.test,
    )
}

fn main() {
    // get_event(1).unwrap().get_ics().write(File::create("test.ics").unwrap());

    rocket::ignite()
        .attach(Template::custom(|engines| {
            engines.handlebars.register_helper("time_custom", Box::new(helper_time_custom_format));
            engines.handlebars.register_helper("add", Box::new(helper_add));
            engines.handlebars.register_helper("previousnavigation", Box::new(helper_previous_navigation));
            engines.handlebars.register_helper("nextnavigation", Box::new(helper_next_navigation));
        }))
        .mount("/", routes![
            event,
            index,
            numbered_index,
            event_ics,
            event_location,
            session,
            location
        ])
        .mount("/public", StaticFiles::from("public/"))
        .launch();
}
