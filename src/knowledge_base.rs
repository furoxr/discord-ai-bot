use anyhow::{anyhow, Result};
use async_openai::{types::CreateEmbeddingRequestArgs, Client as OpenAIClient};
use std::{ops::Deref, path::PathBuf};
use tracing::info;

use qdrant_client::{
    prelude::{Payload, QdrantClient, QdrantClientConfig},
    qdrant::{
        vectors_config::Config, CountPoints, CreateCollection, Distance, PointStruct, SearchPoints,
        VectorParams, VectorsConfig,
    },
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
    let requset = CreateEmbeddingRequestArgs::default()
        .model("text-embedding-ada-002")
        .input(content)
        .build()?;
    let mut response = openai_client.embeddings().create(requset).await?;
    if let Some(data) = response.data.pop() {
        info!("Get embedding length: {:?}", data.embedding.len());

        let count_request = CountPoints {
            collection_name: collection.into(),
            filter: None,
            exact: Some(true),
        };
        let count = qdrant_client
            .count(&count_request)
            .await?
            .result
            .ok_or(anyhow!("No result"))?
            .count;
        info!("Current count in collection: {:?}", count);

        let point = PointStruct::new(count + 1, data.embedding, payload);
        let response = qdrant_client
            .upsert_points(collection, [point].to_vec(), None)
            .await?;
        info!("Upsert response: {:?}", response);
    }

    Ok(())
}

pub async fn query(question: &str, collection_name: &str) -> Result<()> {
    let qdrant_client = KnowledgeClient::new("http://localhost:6334").await?;
    let openai_client = OpenAIClient::new();

    let request = CreateEmbeddingRequestArgs::default()
        .model("text-embedding-ada-002")
        .input(question)
        .build()?;
    let mut response = openai_client.embeddings().create(request).await?;
    if let Some(data) = response.data.pop() {
        info!("Get embedding length: {:?}", data.embedding.len());
        let search = SearchPoints {
            collection_name: collection_name.into(),
            vector: data.embedding,
            filter: None,
            limit: 3,
            with_payload: None,
            params: None,
            score_threshold: None,
            offset: None,
            vector_name: None,
            with_vectors: None,
            read_consistency: None,
        };
        let response = qdrant_client.search_points(&search).await?;
        info!("{:?}", response);
    }
    Ok(())
}

pub async fn clear_collection(collection_name: &str) -> Result<()> {
    let qdrant_client = KnowledgeClient::new("http://localhost:6334").await?;
    let response = qdrant_client
        .delete_collection(collection_name)
        .await?;
    info!("Clear collection response: {:?}", response);
    Ok(())
}