import requests
import argparse

# Initialize parser
parser = argparse.ArgumentParser()
 
# Adding optional argument
parser.add_argument("-t", "--token", help = "Bot token", required=True)
parser.add_argument("-a", "--appid", help = "App id", required=True)
parser.add_argument("-g", "--guid", help = "Guild id", required=True)
 
# Read arguments from command line
args = parser.parse_args()
 
print(args.token)
print(args.appid)
print(args.guid)

response = requests.post(f'https://discord.com/api/v10/applications/{args.appid}/guilds/{args.guid}/commands', headers={'Authorization': f'Bot {args.token}'}, json={"name": "create_event", "description":"Crea un evento", "type": 1}).json()
print(response)