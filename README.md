# discord-ai-bot

[![standard-readme compliant](https://img.shields.io/badge/standard--readme-OK-green.svg?style=flat-square)](https://github.com/RichardLitt/standard-readme)
TODO: Put more badges here.

The discord-ai-bot is a bot service integrated with GPT-3.5, supporting response with customized knowledge.

You could upsert customized data into the Qdrant database. And when the bot was asked a question, it will try to find the most relevant knowledge from Qdrant, and gpt-3.5 will make a response based on this knowledge. 
## Table of Contents

- [discord-ai-bot](#discord-ai-bot)
  - [Table of Contents](#table-of-contents)
  - [Install](#install)
  - [Usage](#usage)
  - [Maintainers](#maintainers)
  - [License](#license)

## Install

Dependencies:
- [protobuf-compiler](https://github.com/protocolbuffers/protobuf/releases)
- [Openssl](https://github.com/openssl/openssl)
- [Qdrant](https://github.com/qdrant/qdrant)

## Usage
You will need to run a [Qdrant database](https://github.com/qdrant/qdrant) locally at present. The config(production.yaml) of Qdrant could check [here](https://github.com/qdrant/qdrant/blob/master/config/config.yaml). 
```
docker pull qdrant/qdrant
docker run -p 6333:6333 -p 6334:6334 \
    -e EDRANT__SERVICE_GRPC_PORT="6334" \
    -v $(pwd)/data:/qdrant/storage \
    -v $(pwd)/production.yaml:/qdrant/config/production.yaml \
    qdrant/qdrant
```

- Start bot service
- Add customized data to qdrant database
- Query knowledge database with a question
- Clear qdrant collection

## Maintainers

[@https://github.com/furoxr](https://github.com/https://github.com/furoxr)


## License

MIT © 2023 Nada Fu
