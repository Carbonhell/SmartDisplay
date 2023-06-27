use std::{collections::HashMap, time::{SystemTime, UNIX_EPOCH}, fs::File, io::Read};

use aws_lambda_events::event::cloudwatch_events::CloudWatchEvent;
use aws_sdk_dynamodb::types::AttributeValue;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use reqwest::{Certificate, Identity};
use serde::{Serialize, Deserialize};
use tracing::log::info;
use serde_json::json;
use url::form_urlencoded::byte_serialize;

const AWS_IOT_ENDPOINT: &str = "";
const DYNAMODB_ACTIVE_EVENTS_TABLE: &str = "active_events";
const FIELD_ID: &str = "id";
const FIELD_TIMESTAMP: &str = "timestamp";
const FIELD_DATETIME: &str = "datetime";
const FIELD_TITLE: &str = "title";
const FIELD_BUILDING: &str = "building";
const FIELD_ROOM: &str = "room";

#[derive(Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub timestamp: u64,
    pub datetime: String,
    pub building: String,
    pub room: String
}

impl Event {
    pub fn from_hashmap(map: &HashMap<String, AttributeValue>) -> Self {
        Self {
            id: map.get(FIELD_ID).unwrap().as_s().unwrap().clone(),
            title: map.get(FIELD_TITLE).unwrap().as_s().unwrap().clone(),
            timestamp: map.get(FIELD_TIMESTAMP).unwrap().as_n().unwrap().parse::<u64>().unwrap(),
            building: map.get(FIELD_BUILDING).unwrap().as_s().unwrap().clone(),
            datetime: map.get(FIELD_DATETIME).unwrap().as_s().unwrap().clone(),
            room: map.get(FIELD_ROOM).unwrap().as_s().unwrap().clone(),
        }
    }
}

pub struct EventList {
    pub events: Vec<Event>
}

impl EventList {
    pub fn new(events: Vec<Event>) -> Self {
        Self {
            events
        }
    }

    pub fn get_future_events(&self) -> Vec<&Event> {
        let current_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let future_events: Vec<&Event> = self.events.iter().filter(|el| el.timestamp > current_unix).collect();
        future_events
    }

    pub async fn send_new_states(&self) {
        let future_events = self.get_future_events();
        
        let mut building_events: HashMap<&String, Vec<&Event>> = HashMap::new();
        let mut room_events: HashMap<(&String, &String), Vec<&Event>> = HashMap::new();
        // We need to create two groups: per building and per room
        for event in future_events {
            let building_group = match building_events.get_mut(&event.building) {
                Some(map) => map,
                None => {
                    building_events.insert(&event.building, Vec::new());
                    building_events.get_mut(&event.building).unwrap()
                }
            };
            building_group.push(event);

            let room_group = match room_events.get_mut(&(&event.building,&event.room)) {
                Some(map) => map,
                None => {
                    room_events.insert((&event.building, &event.room), Vec::new());
                    room_events.get_mut(&(&event.building, &event.room)).unwrap()
                }
            };
            room_group.push(event);
        }
        println!("{}, {}", building_events.len(), room_events.len());
        for (building, mut events) in building_events {
            events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            EventList::send_events(events, building).await;
        }

        for ((building, room), mut events) in room_events {
            events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            EventList::send_events(events, &format!("{}/{}", building, room)).await;
        }
    }
    
    async fn send_events(events: Vec<&Event>, topic: &String) {
        println!("Sending events on topic {}...", topic);
        let urlencoded_topic: String = byte_serialize(topic.as_bytes()).collect();

        let root_ca: Vec<u8> = include_bytes!("../certificates/AmazonRootCA1.pem").to_vec();
        let cert: Vec<u8> = include_bytes!("../certificates/certificate.crt").to_vec();
        let pk: Vec<u8> = include_bytes!("../certificates/private.key").to_vec();

        let client = reqwest::Client::builder()
            .add_root_certificate(Certificate::from_pem(&root_ca).unwrap())
            .identity(Identity::from_pkcs8_pem(&cert, &pk).unwrap())
            .build().unwrap();

        //println!("Sending total: {}, to topic {}, body: {}", format!("{}/topics/{}?qos=1", AWS_IOT_ENDPOINT, urlencoded_topic), urlencoded_topic,serde_json::to_string(&events).unwrap());
        // Retain true to allow clients subscribing in the future to fetch this message
        let response = client.post(format!("{}/topics/{}?qos=1&retain=true", AWS_IOT_ENDPOINT, urlencoded_topic)).json(&events)
            .send()
            .await
            .unwrap();
        println!("{:?}", response);
    }
}

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
/// - https://github.com/aws-samples/serverless-rust-demo/
async fn function_handler(event: LambdaEvent<CloudWatchEvent>) -> Result<serde_json::Value, Error> {
    // Extract some useful information from the request

    let config = aws_config::from_env().endpoint_url("http://localhost:4566").load().await;
    let client = aws_sdk_dynamodb::Client::new(&config);
    let scan_response = client.scan()
        .table_name(DYNAMODB_ACTIVE_EVENTS_TABLE)
        .send()
        .await;
    if let Ok(output) = scan_response {
        let shadow_elements: Vec<Event> = output.items()
            .unwrap() // Why is this an option?
            .iter()
            .map(|el| Event::from_hashmap(el))
            .collect();
        let shadow_elements = EventList::new(shadow_elements);

        shadow_elements.send_new_states().await;
    }
    // println!("OK");
    // let dummy_events = vec![Event{ id: 1, title: "Test".to_string(), timestamp: 1688690076, building: "F3".to_string(), room: "P3".to_string()}];
    // let dummy_events = EventList::new(dummy_events);
    // dummy_events.send_new_states().await;

    Ok(json!({"res":"ok"}))
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
