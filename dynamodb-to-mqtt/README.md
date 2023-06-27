# Prerequisites
You will need to generate a certificate from the AWS IoT console.
Once generated, you need to put the following files with the correct filenames in the certificates folder:
-) `AmazonRootCA1.pem` (you can download this when you create a certificate manually)
-) `certificate.crt`
-) `private.key` CAREFUL! The key you get from the AWS console is in PKCS#1 format. To convert it to PKCS#8, you can use the following command (be sure to change the -in value with the path to your PKCS#1 key):
```sh
openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in pkcs1.key -out private.key
```

You also need to change the AWS_IOT_ENDPOINT const in the main.rs file to the ATS endpoint of your profile MQTT endpoint. You can find it in the AWS console MQTT client.

The AWS endpoint is hardcoded as localhost (for localstack).

# How to run
cargo build --bin dynamodb-to-mqtt
cargo lambda watch

## Simulate cron event
cargo lambda invoke -F debug_event.json --verbose

## DynamoDB data structure

### Access patterns
1) Infrequent write when creating (future) events
2) Frequent read to get the current active events

### Structure

2(1+N) tables:
1) active_events, hosting all the events that should be displayed on things + possibly just expired events
2) 1 (or N, partitioned by date) tables for archived events (not available in the current version)

This way, we can just get all the items from active_events in one trip.
Archival can be either done on a criteria basis when syncing the active events with the thing shadows (eg. when there are >100 expired events) or with a cron lambda (eg. nightly).
We can also use DynamoDB TTL to automatically delete expired events, along with a DynamoDB stream to add logic on deletion to, for example, store the deleted event in Amazon Glacier (very infrequent access)

# Scheduled approach vs sync via DynamoDB stream events

## DynamoDB stream events
Pros:
1) Immediately update shadows with the correct values

Cons:
1) The devices update on a cron basis anyway, so having the shadows be updated in real time doesn't bring any benefit (unless the devices support wakeup from deep sleep through wifi? most likely impossible)
2) Harder to scale (order of event must be respected to properly apply the deltas on the shadows - does dynamodb handle this already?)
3) One lambda invocation per dynamodb change (scales worse)

## Scheduled approach
Pros:
1) Easier to implement: just calculate the new state and apply it

Cons:
1) ?

## Device shadow vs IoT Core MQTT

Shadows:
Pros: ease of syncing state

Cons:
1) Cannot specify a message for a group of things: shadows are meant for direct communication with a specific thing (eg. for firmware updating)

IoT Core MQTT:
Doubts: delta communication vs state transfer

# Useful links


Scheduled CloudWatch events: https://docs.aws.amazon.com/AmazonCloudWatch/latest/events/EventTypes.html#schedule_event_type
Aurora serverless not really serverless: https://www.lastweekinaws.com/blog/the-aurora-serverless-road-not-taken/ "Currently, Aurora..."
DynamoDB intro: https://www.serverless.com/guides/dynamodb
DynamoDB table design: https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/bp-general-nosql-design.html#bp-general-nosql-design-concepts https://www.alexdebrie.com/posts/dynamodb-single-table/ https://aws.amazon.com/it/blogs/compute/creating-a-single-table-design-with-amazon-dynamodb/

# Useful commands
Converting .crt to .pem (not needed?):
openssl x509 -in mycert.crt -out mycert.pem -outform PEM

Converting pkcs#1 (default amazon) to #8:
openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in pkcs1.key -out pkcs8.key

Testing certificate validity with curl
curl --tlsv1.2 --cacert AmazonRootCA1.pem --cert certificate.crt --key private.key --request POST --data "{ \"message\": \"Hello, world\" }" "https://a3p04gu2a31pcg-ats.iot.eu-central-1.amazonaws.com:8443/topics/topic?qos=1"

# Future nice-to-have
1) Caching of the currently retained message per topic to avoid useless mqtt communication when no changes are detected
