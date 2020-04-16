use std::io;

use graphql_client::{GraphQLQuery, Response};

use crate::BACKEND_URL;

#[derive(Clone, Debug, Deserialize, GraphQLQuery, Serialize)]
#[graphql(
schema_path = "resources/schema.graphql",
query_path = "resources/pagination.graphql",
response_derives = "Deserialize, Serialize, Debug"
)]
pub struct Pagination {
    pub event_count: i64,
    pub location_count: i64,
    pub organizer_count: i64,
}

pub(crate) fn get_pagination() -> io::Result<Pagination> {
    let ioerror = |desc| io::Error::new(io::ErrorKind::Other, desc);
    let body = Pagination::build_query(pagination::Variables {});

    let client = reqwest::blocking::Client::new();
    let res = match client.post(BACKEND_URL).json(&body).send() {
        Ok(val) => Ok(val),
        Err(e) => Err(ioerror(format!("{:#?}", e)))
    }?;
    let response: Response<pagination::ResponseData> = res.json().map_err(|e|
        ioerror(format!("Couldn't get successful response from server: {}", e))
    )?;
    let data = response.data.ok_or(
        ioerror(format!("Couldn't get data field from response: {:?}", response.errors.and_then(|x| Some(x.into_iter().map(|x| x.message).collect::<Vec<String>>().join(" | ")))))
    )?;

    Ok(From::from(data.pagination))
}

// TODO: move this
#[derive(Debug)]
pub struct PaginationContext {
    pub limit: u32,
    pub offset: u32,
}

impl Default for PaginationContext {
    fn default() -> Self {
        PaginationContext {
            limit: 100,
            offset: 0,
        }
    }
}

impl From<pagination::PaginationPagination> for Pagination {
    fn from(input: pagination::PaginationPagination) -> Pagination {
        Pagination {
            event_count: input.event_count.parse().unwrap(),
            location_count: input.location_count.parse().unwrap(),
            organizer_count: input.organizer_count.parse().unwrap(),
        }
    }
}
