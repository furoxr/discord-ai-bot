# discord-ai-bot

[![standard-readme compliant](https://img.shields.io/badge/standard--readme-OK-green.svg?style=flat-square)](https://github.com/RichardLitt/standard-readme)

The Discord AI Bot is a bot service that is integrated with GPT-3.5 and supports custom knowledge for responses.

You can add customized data to the Qdrant database using upsert. When the bot is asked a question, it will search for the most relevant knowledge from Qdrant, and GPT-3.5 will generate a response based on this knowledge.
## Table of Contents

- [discord-ai-bot](#discord-ai-bot)
  - [Table of Contents](#table-of-contents)
  - [Build from Source](#build-from-source)
  - [Usage](#usage)
    - [How to start a discord bot service](#how-to-start-a-discord-bot-service)
    - [How to Update knowledge into qdrant database](#how-to-update-knowledge-into-qdrant-database)
    - [How to query the most related knowledge in terminal](#how-to-query-the-most-related-knowledge-in-terminal)
    - [How to clear collection](#how-to-clear-collection)
  - [Maintainers](#maintainers)
  - [License](#license)

## Build from Source

You will need these dependencies:
  - [protobuf-compiler](https://github.com/protocolbuffers/protobuf/releases)
  - [Openssl](https://github.com/openssl/openssl)

## Usage
Currently, you will need to run a [Qdrant database](https://github.com/qdrant/qdrant) locally. You can check the configuration file (production.yaml) of Qdrant [here](https://github.com/qdrant/qdrant/blob/master/config/config.yaml). 
```
docker pull qdrant/qdrant
docker run -p 6333:6333 -p 6334:6334 \
    -e EDRANT__SERVICE_GRPC_PORT="6334" \
    -v $(pwd)/data:/qdrant/storage \
    -v $(pwd)/production.yaml:/qdrant/config/production.yaml \
    qdrant/qdrant
```
### How to start a discord bot service
```
export OPENAI_API_KEY=YOUR_OPENAI_API_KEY
export DISCORD_TOKEN=YOUR_DISCORD_BOT_TOKEN
./discord-ai-bot start
```
### How to Update knowledge into qdrant database
```
export OPENAI_API_KEY=YOUR_OPENAI_API_KEY
export DISCORD_TOKEN=YOUR_DISCORD_BOT_TOKEN
./discord-ai-bot update COLLECTION_NAME DATA_FILE_PATH
```
`COLLECTION_NAME` is the collection name of the qdrant database, which you will upsert knowledge into.
`DATA_FILE_PATH` is the JSON file containing your knowledge, and the format of json file should be like:
```
{
  "title": "Title of the Document",
  "url": "Related url",
  "content": "............."
}
```

### How to query the most related knowledge in terminal
```
export OPENAI_API_KEY=YOUR_OPENAI_API_KEY
export DISCORD_TOKEN=YOUR_DISCORD_BOT_TOKEN
./discord-ai-bot query COLLECTION_NAME YOUR_QUESTION
```
`COLLECTION_NAME` is the collection name of the qdrant database, which you will upsert knowledge into.
Then it will attempt to utilize the embedding service of OpenAI API and the Qdrant database to provide you with a relevant result.


### How to clear collection
```
export OPENAI_API_KEY=YOUR_OPENAI_API_KEY
export DISCORD_TOKEN=YOUR_DISCORD_BOT_TOKEN
./discord-ai-bot query COLLECTION_NAME
```
This operation will clear all data of `COLLECTION_NAME` in the Qdrant database.

## Maintainers

[@nada](https://github.com/furoxr)


## License

MIT © 2023
