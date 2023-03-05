use anyhow::{Result, anyhow};
use async_openai::{Client as OpenAIClient, types::CreateEmbeddingRequestArgs};
use tracing::info;
use std::{ops::Deref, path::PathBuf};

use qdrant_client::{
    prelude::{QdrantClient, QdrantClientConfig, Payload},
    qdrant::{vectors_config::Config, CreateCollection, Distance, VectorParams, VectorsConfig, PointStruct, CountPoints},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgePayload {
    url: String,
    title: String,
    content: String,
}

pub struct KnowledgeClient {
    pub client: QdrantClient,
}

impl KnowledgeClient {
    pub async fn new(url: &str) -> Result<Self> {
        let config = QdrantClientConfig::from_url(url);
        Ok(Self {
            client: QdrantClient::new(Some(config)).await?,
        })
    }
}

impl Deref for KnowledgeClient {
    type Target = QdrantClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

pub async fn upsert_knowledge(file: PathBuf, collection: &str) -> Result<()> {
    let qdrant_client = KnowledgeClient::new("http://localhost:6334").await?;
    let openai_client = OpenAIClient::new();

    if !qdrant_client.has_collection(collection).await? {
        let response = qdrant_client
            .create_collection(&CreateCollection {
                collection_name: collection.into(),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: 1536,
                        distance: Distance::Cosine.into(),
                    })),
                }),
                hnsw_config: None,
                wal_config: None,
                optimizers_config: None,
                shard_number: None,
                on_disk_payload: None,
                timeout: None,
                replication_factor: None,
                write_consistency_factor: None,
                init_from_collection: None,
            })
            .await?;
        info!("Creating collection operation response: {:?}", response);
    }

    // Load JSON content from file
    info!("Loading data from {:?}", &file);
    let text = std::fs::read_to_string(file)?;
    let raw_payload: KnowledgePayload = serde_json::from_str(&text)?;
    let content = raw_payload.content.clone();
    let mut payload = Payload::new();
    info!("Title: {:?}", &raw_payload.title);
    payload.insert("title", raw_payload.title);
    payload.insert("content", raw_payload.content);
    payload.insert("url", raw_payload.url);

    // Get embedding from openai
    let requset = CreateEmbeddingRequestArgs::default().model("text-embedding-ada-002").input(content).build()?;
    let mut response = openai_client.embeddings().create(requset).await?;
    if let Some(data) = response.data.pop() {
        info!("Get embedding length: {:?}", data.embedding.len());

        let count_request = CountPoints { collection_name: collection.into(), filter: None, exact:  Some(true) };
        let count = qdrant_client.count(&count_request).await?.result.ok_or(anyhow!("No result"))?.count;
        info!("Current count in collection: {:?}", count);

        let point = PointStruct::new(count + 1, data.embedding, payload);
        let response = qdrant_client.upsert_points(collection, [point].to_vec(), None).await?;
        info!("Upsert response: {:?}", response);
    }

    Ok(())
}
