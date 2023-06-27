# Serverless discord bot

This lambda is used to respond to command requests from a discord application to offer a modal for event creation. The events are then stored in a DynamoDB document.

## Building
1) cargo build //to compile before watching, else compilation sometimes freezes
1) cargo lambda watch -p 9001
2) ngrok http http://localhost:9001

## Configuration: Discord
Once your Discord Application has been created, copy your public key and paste it in the main.rs PUBLIC_KEY variable.
Once your lambda is active and you have a https ngrok endpoint pointing to it, be sure to go in the Discord Developer portal and edit the "INTERACTIONS ENDPOINT URL" equal to the ngrok endpoint.

## Configuration: AWS
Right now the AWS endpoint is hardcoded in the main.rs file (see: AWS_ENDPOINT_URL, by default set to localstack hosted locally). Be sure to change it as needed.

# TODOs
1) Load configuration vars such as PUBLIC_KEY and the AWS endpoint from the env instead of requiring source code changes