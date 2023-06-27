# Smart display

The objective of this project is to make it easier for people to find out what events will be hosted in a specific building/room through the use of IoT displays. For example, a student may want to know whether a classroom will be occupied or not at a certain hour of the day. Or, a visitor may see an interesting event that will be hosted later in the same day and attend it. It might also be useful for event hosts as a way to advertise their events in frequently accessed hallways.

# Solution
The solution is based on the use of smart IoT displays, associated to a particular room and building, which refresh their own state on a time basis. The state itself is hosted on a persistent storage which can be manipulated through any type of front-end solution, such as a Discord bot.

## Compromises
The main compromises chosen when building this serverless IoT solution were:
1) Energy consumption: to make the hardware maintenance more flexible, it should be battery powered and it should last as much as possible.
2) Logical grouping: the solution should allow having multiple displays associated to a room (for example, if the room has several entrances), and it should allow for a more general building-specific screen that should show the events of all the rooms of the building.

# Implementation

## Cloud-side

The cloud solution uses several Amazon Web Services to satisfy the needs for the various functionalities offered.

### Storage: DynamoDB

DynamoDB has been chosen as a persistent storage in favor of other solutions. The other solutions that were considered were Amazon Aurora Serverless (v2), but it was scrapped due to the Aurora instances not scaling to zero when not in use (see https://www.lastweekinaws.com/blog/the-aurora-serverless-road-not-taken). Even though our data is structured, a NoSQL solution was preferred over a relational one mostly due to pricing and ease of interaction. The pricing allows a prototype version to be pratically free, instead of a RDB one which would require us to pay even for inactive usage.

The data structure has been designed with the idea of single-table design in mind, at least for the initial prototype. This explains the main table referred in the source code (`active_events`). Our data, through, can be reduced to a time series dataset: for this reason, using multiple tables might actually be preferable (see https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/bp-time-series.html). This is detailed in the future improvements section (see Future improvements, point 1).
As for the prototype implemented, expired events aren't cleaned up from the `active_events` table.

### Logic: Lambdas

Logic is handled through the use of two types of lambdas.
The first type handles the creation of events. As of right now, the prototype lambda of this type connects Discord with the use of a custom command and a modal to allow creating an event on DynamoDB.
The second type handles periodic synchronization of events on the AWS IoT Core MQTT queue. Schedule-based synchronization has been preferred over real-time event handling through DynamoDB streams to reduce the amount of messages sent on the MQTT queue. This is a valid tradeoff since the displays do not update in real-time due to compromise #1.

### IoT communication: AWS IoT Core (MQTT)

Communication with real hardware is handled through MQTT. Publishing is done by the second type lambda previously described, through HTTPS. Authentication is handled through a X509 certificate. Things subscribe to their topic of reference (based on building and room) to fetch their desired state.
All messages published on all topics are retained messages: this is needed because the things will most likely be offline when events are published. Therefore, this allows receiving the last sent message during subscription.
The only alternative considered for this step was AWS IoT Device Shadow, but it was scrapped since it's meant for synchronization of configuration options to specific, *single* devices. There seems to be no way to have a shadow shared between multiple devices. Luckily, retained messages are good enough for our needs.
Finally, things connect to the MQTT queue through the MQTT protocol, which allows consuming less energy due to less overhead (instead of HTTPS). Authentication is, once again, done through X509 certificates.


## Hardware side

The hardware chosen for the project is:
1) [FireBeetle 2 ESP32-E IoT Microcontroller](https://thepihut.com/products/firebeetle-esp32-e-iot-microcontroller-supports-wi-fi-bluetooth?variant=39493908201667)
2) [E-Ink Display Raw Panel 5.83" (648x480)](https://thepihut.com/products/e-ink-display-module-5-83-648x480?variant=39695870787779)
3) [Universal e-Paper Raw Panel Driver HAT](https://thepihut.com/products/universal-e-paper-raw-panel-driver-hat?variant=32051318652990)

All the choices have been made with energy consumption in mind. The ESP32-E board offers a deep-sleep mode which allows consuming around 10µA. This mode isn't used in the current prototype, but it can be added when the board isn't refreshing its own state through the MQTT queue (aka most of the time!). The display has been chosen due to the static nature of the content to be displayed: the long refresh rate isn't an issue. Instead, the screen doesn't consume power when displaying an image (unlike LCD screens), only when refreshing it.

### Simulation

Simulating the hardware is done through Wokwi. Currently, the only e-paper module offered is the [296x128, 2.9inch E-Ink display module](https://www.waveshare.com/2.9inch-e-paper-module.htm).


# Building & Running

To build the project, you will need to prepare the required services, build two lambdas and deploy them, and start your hardware (simulated or not).

## Prerequisites
1) Localstack
2) [Ngrok](https://ngrok.com/download) (needed for Discord API interaction)
3) A [Discord](https://discord.com/) account
4) (Optional) Python (only used to create a Discord Bot command, basically a HTTP request you can also do with curl)

## Environment

Running the Amazon cloud services locally can be done through [Localstack](https://docs.localstack.cloud/getting-started/).
To start a Localstack container, run:
```sh
docker run --rm -it -p 4566:4566 -p 4510-4559:4510-4559 localstack/localstack
```

### DynamoDB

Table creation:
````sh`
aws dynamodb create-table --table-name active_events --key-schema AttributeName=id,KeyType=HASH --attribute-definitions AttributeName=id,AttributeType=S --billing-mode PAY_PER_REQUEST --region eu-central-1 --endpoint-url http://localhost:4566
```

To verify that the table has been successfully created, you can run the following command:
```sh
aws dynamodb list-tables --endpoint-url http://localhost:4566
```

### Discord
Create an application in the [Discord developer panel](https://discord.com/developers/applications). You will also need a server where you can add your bot to be able to interact with it.
When creating the application, write down the **App ID** and the **Token**. When you create the discord server, note down the **Guild ID** too.
To create the command that will be used by users to open up the modal, run the following python script:
```sh
python ./scripts/create_command.py
```
It will tell you the arguments needed. Call it again with all the arguments properly filled.

### The rest
For the rest of the configuration, please view the README file in each project folder.

# Future improvements
1) Implement archiving of expired events. A hypothetical solution might use DynamoDB events and the TTL field to automatically delete expired events from `active_events` and move them somewhere else. Such a destination could be either a differently scaled DynamoDB table (low writes and low reads), or even a S3 bucket or a Glacier storage (depending on future possible uses of this data).
2) Use AWS SAM or an alternative to make it easier to deploy the cloud services
