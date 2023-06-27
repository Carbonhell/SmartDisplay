use std::collections::{HashMap, HashSet};

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, NaiveDateTime};
use ed25519_dalek::{PublicKey, Signature, Verifier};
use lambda_http::{
    aws_lambda_events::{
        serde::{Deserialize, Serialize},
        serde_json,
    },
    http::{response, StatusCode},
    run, service_fn, Body, Error, Request, RequestExt, RequestPayloadExt, Response,
};
use serde_json::{json, Value, Map};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

const PUBLIC_KEY: &[u8] = b"";
const DYNAMODB_TABLE_NAME: &str = "active_events";
const AWS_ENDPOINT_URL: &str = "http://localhost:4566";

// Discord interaction types
const PING: u64 = 1;
const APPLICATION_COMMAND: u64 = 2;
const MESSAGE_COMPONENT: u64 = 3;
const MODAL_SUBMIT: u64 = 5;

// TODO define fields
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct DiscordPayload;

fn verify_signature(event: &Request) -> Result<(), Error> {
    let signature = event.headers().get("X-Signature-Ed25519").ok_or("Err")?;
    let timestamp = event
        .headers()
        .get("X-Signature-Timestamp")
        .ok_or("Error")?;
    let body = event.body();
    match body {
        Body::Text(body) => {
            println!("Syntax valid");
            let public_key = hex::decode(PUBLIC_KEY)?;
            let public_key = PublicKey::from_bytes(&public_key)?;
            println!("Public key built");
            let message = [timestamp.as_bytes(), body.as_bytes()].concat();
            let signature = hex::decode(signature)?;
            let signature = Signature::from_bytes(&signature)?;
            println!("Signature built!");
            public_key.verify_strict(&message, &signature)?;
            Ok(())
        }
        _ => Err("Error".into()),
    }
}
/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
async fn function_handler(event: Request) -> Result<Response<String>, Error> {
    println!("Called!");

    if let Err(_) = verify_signature(&event) {
        println!("Signature failed verification");
        return Ok(response(StatusCode::UNAUTHORIZED, json!({})));
    }

    // Extract some useful information from the request
    // let who = event
    //     .query_string_parameters_ref()
    //     .and_then(|params| params.first("name"))
    //     .unwrap_or("world");
    // let message = format!("Hello {who}, this is an AWS Lambda HTTP request");
    let discord_event: HashMap<String, Value> = match event.payload() {
        Ok(Some(discord_event)) => discord_event,
        Ok(None) => {
            warn!("Missing discord_event in request body");
            return Ok(response(
                StatusCode::BAD_REQUEST,
                json!({"message": "Missing discord_event in request body"}),
            ));
        }
        Err(err) => {
            warn!("Failed to parse discord_event from request body: {}", err);
            return Ok(response(
                StatusCode::BAD_REQUEST,
                json!({"message": "Failed to parse discord_event from request body"}),
            ));
        }
    };
    info!("Parsed discord_event: {:?}", discord_event);
    let discord_type = discord_event.get("type");
    if let Some(discord_type) = discord_type {
        if let Value::Number(type_id) = discord_type {
            let type_id = type_id.as_u64().unwrap();
            if type_id == PING {
                return Ok(response(StatusCode::OK, json!({"type":1})));
            }

            if type_id == APPLICATION_COMMAND {
                // TODO handle fetching credentials when lambda's deployed
                let config = ::aws_config::load_from_env().await;
                let client = aws_sdk_iot::Client::new(&config);
                let things = client.list_things().send().await?;
                let mut options = HashMap::<String, HashSet<String>>::new();
                // Build a map of building -> [rooms]
                for thing in things.things().unwrap() {
                    if let Some(attributes) = thing.attributes() {
                        if let Some(building) = attributes.get("building") {
                            let rooms = options
                                .entry(building.to_owned())
                                .or_insert_with(|| HashSet::<String>::new());
                            if let Some(room) = attributes.get("room") {
                                rooms.insert(room.to_owned());
                            }
                        }
                    }
                }
                println!("Options calculated: {:?}", options);
                // Reduce the map to a vector of options
                let mut dropdown_options = Vec::<String>::new();
                for (building, rooms) in options.iter() {
                    for room in rooms {
                        let mut option = building.to_owned();
                        option.push_str(" - ");
                        option.push_str(room);
                        dropdown_options.push(option);
                    }
                }
                let formatted_dropdown_options = dropdown_options
                    .iter()
                    .map(|e| {
                        let mut map = HashMap::<&str, &String>::new();
                        map.insert("label", e);
                        map.insert("value", e);
                        map
                    })
                    .collect::<Vec<HashMap<&str, &String>>>();
                println!("Options formatted: {:?}", dropdown_options);
                println!("Test: {:?}", serde_json::to_string(&dropdown_options));
                println!(
                    "Test2: {:?}",
                    serde_json::to_string(&formatted_dropdown_options)
                );

                println!("Responding to command!");
                return Ok(response(
                    StatusCode::OK,
                    json!({
                        "type":4,
                        "data": {
                            "content":"Seleziona la stanza dove si terrà l'evento per procedere alla compilazione delle informazioni necessarie.",
                            "components": [
                                {
                                "type":1,
                                "components":[
                                {
                                "type": 3,
                                "custom_id":"selected_room",
                                "options": formatted_dropdown_options
                            }]
                        }
                        ]
                        }
                    }),
                ));
            }
            if type_id == MESSAGE_COMPONENT {
                println!("Message component");
                let data = discord_event.get("data").ok_or("Missing data")?;
                if let Value::Object(data) = data {
                    println!("Data found");
                    let custom_id = data.get("custom_id").ok_or("Missing custom id")?;
                    if let Value::String(custom_id) = custom_id {
                        println!("custom_id found");
                        if custom_id != "selected_room" {
                            return Err("Unknown field".into());
                        }
                        let room = data.get("values").ok_or("Missing values")?;
                        if let Value::Array(room) = room {
                            println!("Values found");
                            let room = room.first().ok_or("Missing selected value")?;
                            if let Value::String(room) = room {
                                println!("Room found: {}", room);
                                return Ok(response(
                                    StatusCode::OK,
                                    json!({
                                                "type":9,
                                                "data": {
                                                  "custom_id":"event_info",
                                                  "title": "test",
                                                  "components": [{
                                                    "type":1,
                                                    "components": [{
                                                      "type": 4,
                                                      "custom_id":"title",
                                                      "label": "Titolo",
                                                      "style":1
                                                    }]
                                                   },{
                                                    "type":1,
                                                    "components": [{
                                                      "type": 4,
                                                      "custom_id":"description",
                                                      "label": "Descrizione",
                                                      "style":2
                                                    }]
                                                },{
                                                    "type":1,
                                                    "components": [{
                                                      "type": 4,
                                                      "custom_id":"datetime",
                                                      "label": "Data e ora (Formato: YYYY-MM-DD HH:mm)",
                                                      "style":1
                                                    }]
                                                   },{
                                                    "type":1,
                                                    "components": [{
                                                      "type": 4,
                                                      "custom_id":"room",
                                                      "label": "Stanza",
                                                      "style":1,
                                                      "value": room
                                                    }]
                                                }]
                                    }}),
                                ));
                            }
                        }
                    }
                }
            }
            if type_id == MODAL_SUBMIT {
                let event_data = parse_modal_event(&event)?;
                println!("Event data: {:?}", event_data);
                let datetime = NaiveDateTime::parse_from_str(&event_data.datetime, "%Y-%m-%d %H:%M")?;
                
                let config = aws_config::from_env().endpoint_url(AWS_ENDPOINT_URL).load().await;
                let client = aws_sdk_dynamodb::Client::new(&config);
                let str_elems: Vec<&str> = event_data.room.split(" - ").collect();
                if let [building, room] = str_elems[..] {
                    client.put_item()
                    .table_name(DYNAMODB_TABLE_NAME)
                    .item("id", AttributeValue::S(Uuid::new_v4().to_string()))
                    .item("title", AttributeValue::S(event_data.title))
                    .item("description", AttributeValue::S(event_data.description))
                    .item("datetime", AttributeValue::S(event_data.datetime))
                    .item("timestamp", AttributeValue::N(datetime.timestamp().to_string()))
                    .item("building", AttributeValue::S(building.to_string()))
                    .item("room", AttributeValue::S(room.to_string()))
                    .send()
                    .await?;

                    return Ok(response(
                        StatusCode::OK,
                        json!({
                            "type":4,
                            "data": {
                                "content": "Evento salvato correttamente."
                            }
                        })));
                }
                return Ok(response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "type":4,
                        "data": {
                            "content": "Qualcosa è andato storto!"
                        }
                    })))
            }
        }
    }

    // if discord_event.get("type") == 1 {
    //     return Ok(response(StatusCode::OK, json!({"type":1})));
    // }
    Ok(response(StatusCode::BAD_REQUEST, json!({})))

    // Return something that implements IntoResponse.
    // It will be serialized to the right response event automatically by the runtime
    // let resp = Response::builder()
    //     .status(200)
    //     .header("content-type", "text/html")
    //     .body("test".to_string())
    //     .map_err(Box::new)?;
    // Ok(resp)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        // disable printing the name of the module in every log line.
        .with_target(false)
        // disabling time is handy because CloudWatch will add the ingestion time.
        .without_time()
        .init();

    run(service_fn(function_handler)).await
}

/// HTTP Response with a JSON payload
fn response(status_code: StatusCode, body: Value) -> Response<String> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .unwrap()
}
#[derive(Serialize, Deserialize)]
struct DiscordComponent {
    #[serde(rename="type")]
    pub type_id: u64,
    pub components: Option<Vec<DiscordComponent>>,
    pub custom_id: Option<String>,
    pub value: Option<String>
}
#[derive(Serialize, Deserialize)]
struct Modal {
    pub custom_id: String,
    pub components: Vec<DiscordComponent>
}
#[derive(Serialize, Deserialize)]
struct ModalEventData {
    pub data: Modal
}

#[derive(Debug)]
struct EventData {
    pub title: String,
    pub description: String,
    pub datetime: String,
    pub room: String
}

fn parse_modal_event(event: &Request) -> Result<EventData, &str>{
    let modal_event: ModalEventData = event.payload().unwrap().ok_or("Wrong data")?;
    if modal_event.data.custom_id != "event_info" {
        return Err("Wrong modal custom_id");
    }
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut datetime: Option<String> = None;
    let mut room: Option<String> = None;
    for action_row in modal_event.data.components {
        for field in action_row.components.unwrap() {
            if let Some(custom_id) = field.custom_id {
                match custom_id.as_str() {
                    "title" => {title = field.value},
                    "description" => {description = field.value},
                    "datetime" => {datetime = field.value},
                    "room" => {room = field.value}
                    _ => ()
                }
            }
        }
    }
    if let (Some(title), Some(description), Some(datetime), Some(room)) = (title, description, datetime, room) {
        return Ok(EventData{title, description, datetime, room})
    }
    Err("Missing info")
}

// fn parse_modal_data(data: &Map<String, Value>) {
//     let modal_id = data.get("custom_id").ok_or("Missing modal custom_id")?;
//     if let Value::String(modal_id) = modal_id {
//         if modal_id != "event_info" {
//             return Err(format!("Wrong modal custom_id: {}", modal_id));
//         }

//         let action_rows = data.get("components").ok_or("Missing modal components (action rows")?;
//         if let Value::Array(action_rows) = action_rows {
//             for action_row in action_rows {
//                 if let Value::Object(action_row) = action_row {

//                 }
//             }
//         }
//     }
// }