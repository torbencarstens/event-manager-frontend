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
use rocket::request::{FlashMessage, Form, FromRequest, Outcome};
use rocket::response::{Content, Flash, Redirect, Stream};
use rocket_contrib::serve::StaticFiles;
use rocket_contrib::templates::handlebars::{Context, Handlebars, Helper, HelperResult, JsonRender, JsonValue, Output, Renderable, RenderContext, RenderError};
use rocket_contrib::templates::handlebars::template::{HelperTemplate, Parameter, TemplateElement};
use rocket_contrib::templates::Template;

use events_frontend::{backend_url, PaginationContext};
use events_frontend::helper::*;

#[derive(Clone, Debug, Deserialize, GraphQLQuery, Serialize)]
#[graphql(
schema_path = "resources/schema.graphql",
query_path = "resources/location.graphql",
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

#[derive(Clone, Debug, Deserialize, FromForm, GraphQLQuery, Serialize)]
#[graphql(
schema_path = "resources/schema.graphql",
query_path = "resources/mutations/location.graphql",
response_derives = "Deserialize, Serialize, Debug"
)]
struct LocationMutation {
    name: String,
    website: Option<String>,
    street: String,
    street_number: i64,
    city: String,
    postal_code: i64,
    country: String,
    building: Option<String>,
    maps_link: String,
}

impl LocationMutation {
    fn add(self) -> io::Result<Location> {
        let ioerror = |desc| io::Error::new(io::ErrorKind::Other, desc);
        let body = LocationMutation::build_query(self.into());


        let response = match reqwest::blocking::Client::new()
            .post(&backend_url())
            .json(&body)
            .send() {
            Ok(val) => Ok(val),
            Err(e) => Err(ioerror(format!("{:#?}", e)))
        }?;
        let response: Response<location_mutation::ResponseData> = response.json().map_err(|e|
            ioerror(format!("Couldn't get successful response from server: {}", e))
        )?;
        let data = response.data.ok_or(
            ioerror(format!("Couldn't get data field from response: {:?}", response.errors.and_then(|x| Some(x.into_iter().map(|x| x.message).collect::<Vec<String>>().join(" | ")))))
        )?;

        Ok(data
            .location
            .into())
    }
}

impl Into<Location> for location_mutation::LocationMutationLocation {
    fn into(self) -> Location {
        Location {
            id: self.id,
            name: self.name,
            website: self.website,
            street: self.street,
            street_number: self.street_number as i32,
            city: self.city,
            postal_code: self.postal_code as i32,
            country: self.country,
            building: self.building,
            maps_link: self.maps_link,
        }
    }
}

impl Into<location_mutation::Variables> for LocationMutation {
    fn into(self) -> location_mutation::Variables {
        location_mutation::Variables {
            input: location_mutation::LocationInput {
                name: self.name,
                website: self.website,
                street: self.street,
                street_number: self.street_number,
                city: self.city,
                postal_code: self.postal_code,
                country: self.country,
                building: self.building,
                maps_link: self.maps_link,
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, GraphQLQuery, Serialize)]
#[graphql(
schema_path = "resources/schema.graphql",
query_path = "resources/tags.graphql",
response_derives = "Deserialize, Serialize, Debug"
)]
struct Tag {
    id: i64,
    name: String,
    description: Option<String>,
    events: Vec<Event>,
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

impl From<tag::TagTag> for Tag {
    fn from(tag: tag::TagTag) -> Self {
        Tag {
            id: tag.id,
            name: tag.name,
            description: tag.description,
            events: tag.events.into_iter().map(|event: tag::TagTagEvents| Event {
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
                tags: event
                    .tags
                    .into_iter()
                    .map(|tag| InnerEventTag {
                        id: tag.id,
                        name: tag.name,
                        description: tag.description,
                    })
                    .collect(),
            }).collect::<Vec<Event>>(),
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
            tags: event
                .tags
                .into_iter()
                .map(|tag| tag.into())
                .collect(),
        }
    }
}

impl Into<InnerEventTag> for event::EventEventTags {
    fn into(self) -> InnerEventTag {
        InnerEventTag {
            id: self.id,
            name: self.name,
            description: self.description,
        }
    }
}


struct TagInput {
    id: Option<i64>,
    name: Option<String>,
    description: Option<String>,
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

impl Into<tag::Variables> for TagInput {
    fn into(self) -> tag::Variables {
        let default_pagination = PaginationContext::default();

        tag::Variables {
            constraints: Some(tag::Constraints {
                limit: default_pagination.limit.to_string(),
                offset: default_pagination.offset.to_string(),
            }),
            input: Some(tag::TagQuery
            {
                id: self.id,
                name: self.name,
                description: self.description,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InnerEventTag {
    id: i64,
    name: String,
    description: Option<String>,
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
    tags: Vec<InnerEventTag>,
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


fn get_tag(id: i64) -> io::Result<Tag> {
    get_tags(TagInput {
        id: Some(id),
        name: None,
        description: None,
    }.into())?
        .pop()
        .ok_or(io::Error::from(io::ErrorKind::NotFound))
}

fn get_tags(variables: tag::Variables) -> io::Result<Vec<Tag>> {
    let ioerror = |desc| io::Error::new(io::ErrorKind::Other, desc);
    let body = Tag::build_query(variables);

    let client = reqwest::blocking::Client::new();
    let res = match client.post(&backend_url()).json(&body).send() {
        Ok(val) => Ok(val),
        Err(e) => Err(ioerror(format!("{:#?}", e)))
    }?;
    let response: Response<tag::ResponseData> = res.json().map_err(|e|
        ioerror(format!("Couldn't get successful response from server: {}", e))
    )?;
    let data = response.data.ok_or(
        ioerror(format!("Couldn't get data field from response: {:?}", response.errors.and_then(|x| Some(x.into_iter().map(|x| x.message).collect::<Vec<String>>().join(" | ")))))
    )?;

    Ok(data
        .tag
        .into_iter()
        .map(From::from)
        .collect::<Vec<Tag>>())
}

fn get_locations(variables: location::Variables) -> io::Result<Vec<Location>> {
    let ioerror = |desc| io::Error::new(io::ErrorKind::Other, desc);
    let body = Location::build_query(variables);

    let client = reqwest::blocking::Client::new();
    let res = match client.post(&backend_url()).json(&body).send() {
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
    let res = match client.post(&backend_url()).json(&body).send() {
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
struct LocationListTemplateContext<'a> {
    title: String,
    parent: &'a str,
    page_id: u32,
    locations: Vec<Location>,
}

#[derive(Deserialize, Serialize)]
struct EventListTemplateContext<'a> {
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
    Redirect::to("/events/1") // there has to be a better option
}

#[get("/locations")]
fn locations(flash: Option<FlashMessage<'_, '_>>) -> Redirect {
    Redirect::to("/locations/1") // there has to be a better option
}

#[get("/locations/<id>")]
fn locations_numbered(id: Option<u32>) -> Template {
    let page_id = max(1, id.unwrap_or(1));
    let pagination = if page_id > 1 {
        let mut context = PaginationContext::default();
        context.offset = context.limit * (page_id - 1);
        context
    } else {
        PaginationContext::default()
    };

    let mut input: location::Variables = LocationInput {
        id: None,
        name: None,
        website: None,
        street: None,
        street_number: None,
        city: None,
        postal_code: None,
        country: None,
        building: None,
        maps_link: None,
    }.into();
    input.constraints = Some(location::Constraints {
        limit: pagination.limit.to_string(),
        offset: pagination.offset.to_string(),
    });
    let locations = get_locations(input).unwrap(); // TODO

    Template::render("locations", LocationListTemplateContext {
        title: "Locations".to_string(),
        parent: "layout",
        page_id,
        locations,
    })
}

#[get("/events/<id>")]
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

    let context = EventListTemplateContext {
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

#[derive(Debug, Deserialize, Serialize)]
struct TagTemplateContext<'a> {
    title: String,
    parent: &'a str,
    tag: Tag,
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

#[get("/tag/<id>")]
fn tag(id: i64) -> Template {
    let tag = get_tag(id).unwrap(); // TODO

    let context = TagTemplateContext {
        title: tag.name.clone(),
        parent: "layout",
        tag,
    };

    Template::render("tag", context)
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

#[get("/location/<id>/edit")]
fn location_edit(id: i64) -> Template {
    let location = if id != 0 {
        get_location(id).unwrap() // TODO
    } else {
        Location {
            id: 0,
            name: "".to_string(),
            website: None,
            street: "".to_string(),
            street_number: 0,
            city: "".to_string(),
            postal_code: 0,
            country: "".to_string(),
            building: None,
            maps_link: "".to_string(),
        }
    };

    let context = LocationTemplateContext {
        title: location.name.clone(),
        parent: "layout",
        location,
        events: vec![],
    };

    Template::render("edit/location", context)
}

#[post("/location/<id>/submit", data = "<location>")]
fn location_submit(id: Option<i64>, location: Form<LocationMutation>) -> Redirect {
    // let location = get_location(id.unwrap()).unwrap(); // TODO
    //
    // let context = LocationTemplateContext {
    //     title: location.name.clone(),
    //     parent: "layout",
    //     location,
    //     events: vec![],
    // };

    let result = location.0.add();
    println!("r: {:#?}", result);

    // Redirect::to("{TODO}")
    Redirect::to("/locations")
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
            engines.handlebars.register_helper("format_description", Box::new(helper_format_description));
            engines.handlebars.register_helper("unwrap_or", Box::new(helper_unwrap_or));
        }))
        .mount("/", routes![
            event,
            index,
            numbered_index,
            event_ics,
            event_location,
            session,
            location,
            tag,
            locations,
            locations_numbered,
            location_edit,
            location_submit,
        ])
        .mount("/public", StaticFiles::from("public/"))
        .launch();
}
