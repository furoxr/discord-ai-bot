use anyhow::{anyhow, Result};
use async_openai::{types::CreateEmbeddingRequestArgs, Client as OpenAIClient};
use std::{ops::Deref, path::PathBuf};
use tracing::{info, trace};
use uuid::Uuid;

use qdrant_client::{
    prelude::{Payload, QdrantClient, QdrantClientConfig},
    qdrant::{
        vectors_config::Config, with_payload_selector::SelectorOptions, CountPoints,
        CreateCollection, Distance, PointStruct, ScoredPoint, SearchPoints, VectorParams,
        VectorsConfig, WithPayloadSelector,
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

impl KnowledgeClient {
    pub async fn query_knowledge(
        &self,
        collection_name: &str,
        embedding: Vec<f32>,
        score_threshold: Option<f32>,
    ) -> Result<Vec<ScoredPoint>> {
        let points = self
            .search_points(&SearchPoints {
                collection_name: collection_name.into(),
                vector: embedding,
                limit: 3,
                with_payload: Some(WithPayloadSelector {
                    selector_options: Some(SelectorOptions::Enable(true)),
                }),
                score_threshold,
                ..Default::default()
            })
            .await?;
        if points.result.is_empty() {
            return Err(anyhow!("No knowledge found"));
        }
        trace!("query_knowledge costs: {}", points.time);
        Ok(points.result)
    }

    pub async fn create_knowledge_collection(&self, collection_name: &str) -> Result<()> {
        if self.has_collection(collection_name).await? {
            return Ok(());
        }
        self.create_collection(&CreateCollection {
            collection_name: collection_name.into(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(VectorParams {
                    size: 1536,
                    distance: Distance::Cosine.into(),
                })),
            }),
            ..Default::default()
        })
        .await?;
        Ok(())
    }

    pub async fn upsert_knowledge(
        &self,
        collection_name: &str,
        knowledge: KnowledgePayload,
        embedding: Vec<f32>,
    ) -> Result<()> {
        let mut payload = Payload::new();
        trace!("Upserting knowledge: {:?}", &knowledge.title);
        payload.insert("title", knowledge.title);
        payload.insert("content", knowledge.content);
        payload.insert("url", knowledge.url);
        let point = PointStruct::new(Uuid::new_v4().to_string(), embedding, payload);
        self.upsert_points(collection_name, [point].to_vec(), None)
            .await?;
        Ok(())
    }
}

impl Deref for KnowledgeClient {
    type Target = QdrantClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

pub async fn upsert_knowledge(qdrant_url: &str, file: PathBuf, collection: &str) -> Result<()> {
    let qdrant_client = KnowledgeClient::new(qdrant_url).await?;
    let openai_client = OpenAIClient::new();

    if !qdrant_client.has_collection(collection).await? {
        let response = qdrant_client
            .create_knowledge_collection("darwinia")
            .await?;
        info!("Creating collection operation response: {:?}", response);
    }

    // Load JSON content from file
    info!("Loading data from {:?}", &file);
    let text = std::fs::read_to_string(file)?;
    let raw_payload: KnowledgePayload = serde_json::from_str(&text)?;
    let content = raw_payload.content.clone();

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
            .ok_or_else(|| anyhow!("No result"))?
            .count;
        info!("Current count in collection: {:?}", count);

        let response = qdrant_client
            .upsert_knowledge(collection, raw_payload, data.embedding)
            .await?;
        info!("Upsert response: {:?}", response);
    }

    Ok(())
}

pub async fn query(qdrant_url: &str, question: &str, collection_name: &str) -> Result<()> {
    let qdrant_client = KnowledgeClient::new(qdrant_url).await?;
    let openai_client = OpenAIClient::new();

    let request = CreateEmbeddingRequestArgs::default()
        .model("text-embedding-ada-002")
        .input(question)
        .build()?;
    let mut response = openai_client.embeddings().create(request).await?;
    if let Some(data) = response.data.pop() {
        info!("Get embedding length: {:?}", data.embedding.len());
        let response = qdrant_client
            .query_knowledge(collection_name, data.embedding, None)
            .await?;
        info!("{:?}", response);
    }
    Ok(())
}

pub async fn clear_collection(qdrant_url: &str, collection_name: &str) -> Result<()> {
    let qdrant_client = KnowledgeClient::new(qdrant_url).await?;
    let response = qdrant_client.delete_collection(collection_name).await?;
    info!("Clear collection response: {:?}", response);
    Ok(())
}
