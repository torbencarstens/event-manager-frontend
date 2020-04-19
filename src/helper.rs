use chrono::NaiveDateTime;
use rocket_contrib::templates::handlebars::{Context, Handlebars, Helper, HelperResult, JsonValue, Output, RenderContext};
use rocket_contrib::templates::handlebars::JsonRender;
use rocket_contrib::templates::handlebars::template::{Parameter, TemplateElement};

use crate::{get_pagination, PaginationContext};

pub fn helper_add(h: &Helper, _: &Handlebars, _: &Context, _: &mut RenderContext, out: &mut dyn Output) -> HelperResult {
    out.write(JsonValue::from(
        h.param(0).unwrap().value().as_i64().unwrap() +
            h.param(1).unwrap().value().as_i64().unwrap()
    ).render().as_ref())?;
    Ok(())
}

pub fn helper_previous_navigation(h: &Helper, _: &Handlebars, context: &Context, rc: &mut RenderContext, out: &mut dyn Output) -> HelperResult {
    let default = serde_json::Map::new();
    let page_id = context
        .data()
        .as_object()
        .unwrap_or(&default)
        .get("page_id")
        .and_then(|x|
            x.as_i64())
        .unwrap_or(1i64);
    let link_prefix = if let Some(param) = h.param(0) {
        param.value().as_str().unwrap_or("")
    } else {
        ""
    };

    let value = if page_id != 1 {
        format!(r#"<li class="page-item background-secondary">
                    <a class="page-link text-color background-secondary" href="/{}{}"
                       aria-label="Previous">
                        <span aria-hidden="true">&laquo;</span>
                        <span class="sr-only">Previous</span>
                    </a>
                </li>
                <li class="page-item background-secondary"><a class="page-link text-color background-secondary"
                                                              href="/{}{}">{}</a>
                </li>"#, link_prefix, page_id - 1, link_prefix, page_id - 1, page_id - 1)
    } else {
        String::new()
    };

    out.write(JsonValue::String(value).render().as_ref())?;
    Ok(())
}

// TODO: actually implement this
// we need a max_page for this
pub fn helper_next_navigation(h: &Helper, _: &Handlebars, context: &Context, rc: &mut RenderContext, out: &mut dyn Output) -> HelperResult {
    let default = serde_json::Map::new();
    let context = context
        .data()
        .as_object()
        .unwrap_or(&default);

    let mut value = match h.template() {
        None => {
            // this never happens unless the user forgets to add an inline template inside next, IDC
            String::new()
        }
        Some(template) => {
            template
                .elements
                .clone()
                .into_iter()
                .map(|element|
                    match element {
                        TemplateElement::RawString(string) => {
                            string
                        }
                        TemplateElement::HelperExpression(expression) => {
                            match expression.name.as_ref() {
                                "add" => {
                                    // TODO: throw error
                                    let first_param = expression.params.get(0).unwrap();
                                    let second_param = expression.params.get(1).unwrap();

                                    let x = match first_param {
                                        Parameter::Name(name) => {
                                            // TODO: throw error
                                            context.get(name.as_str()).unwrap_or(&JsonValue::from(0i64)).as_i64().unwrap()
                                        }
                                        Parameter::Literal(x) => {
                                            x.as_i64().unwrap()
                                        }
                                        _ => {
                                            // TODO: throw error
                                            0i64
                                        }
                                    };

                                    let y = match second_param {
                                        Parameter::Name(name) => {
                                            // TODO: throw error
                                            context.get(name.as_str()).unwrap_or(&JsonValue::from(0i64)).as_i64().unwrap()
                                        }
                                        Parameter::Literal(x) => {
                                            x.as_i64().unwrap()
                                        }
                                        _ => {
                                            // TODO: throw error
                                            0i64
                                        }
                                    };
                                    format!("{}", x + y)
                                }
                                _ => String::new()
                            }
                        }
                        _ => String::new(),
                    }
                )
                .collect::<Vec<String>>()
                .join("\n")
        }
    };

    let page_id = context
        .get("page_id")
        .and_then(|x|
            x.as_i64())
        .unwrap_or(1i64);
    let pagination_context = h
        .param(0)
        .and_then(|param|
            param.value().as_str())
        .unwrap_or("events");
    let events_count = get_pagination()
        .and_then(|x|
            Ok(match pagination_context {
                "locations" => x.location_count,
                "organizers" => x.organizer_count,
                _ => x.event_count
            }))
        .unwrap_or(PaginationContext::default().limit as i64);

    if page_id >= (events_count / PaginationContext::default().limit as i64) {
        value = String::new();
    }

    out.write(JsonValue::String(value).render().as_ref())?;
    Ok(())
}

pub fn helper_time_custom_format(h: &Helper, _: &Handlebars, context: &Context, _: &mut RenderContext, out: &mut dyn Output) -> HelperResult {
    let param = h.param(0).unwrap();
    let format_param = h.param(1).unwrap();

    let value = param.value().as_str().unwrap();
    let format_string = format_param.value().as_str().unwrap();
    let value = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").unwrap(); // TODO

    out.write(JsonValue::String(value.format(format_string).to_string()).render().as_ref())?;
    Ok(())
}
