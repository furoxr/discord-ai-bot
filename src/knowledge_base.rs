use anyhow::{anyhow, Result};
use async_openai::{types::CreateEmbeddingRequestArgs, Client as OpenAIClient};
use std::{collections::HashMap, ops::Deref, path::PathBuf};
use tracing::{error, info, trace};
use uuid::Uuid;

use qdrant_client::{
    prelude::{Payload, QdrantClient, QdrantClientConfig},
    qdrant::{
        value::Kind, vectors_config::Config, with_payload_selector::SelectorOptions,
        CollectionOperationResponse, CountPoints, CreateCollection, Distance, PointStruct,
        PointsOperationResponse, SearchPoints, Value, VectorParams, VectorsConfig,
        WithPayloadSelector,
    },
};
use serde::{Deserialize, Serialize};

use crate::helper::try_match;

#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgePayload {
    pub url: String,
    pub title: String,
    pub content: String,
}

impl TryFrom<HashMap<String, Value>> for KnowledgePayload {
    type Error = anyhow::Error;

    fn try_from(value: HashMap<String, Value>) -> Result<Self, Self::Error> {
        let url = try_match!(value, "url", StringValue);
        let title = try_match!(value, "title", StringValue);
        let content = try_match!(value, "content", StringValue);
        Ok(Self {
            url,
            title,
            content,
        })
    }
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
    ) -> Result<Vec<KnowledgePayload>> {
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
        let result = points
            .result
            .into_iter()
            .map(|x| x.payload.try_into())
            .collect::<Result<Vec<KnowledgePayload>>>()?;
        Ok(result)
    }

    pub async fn create_knowledge_collection(
        &self,
        collection_name: &str,
    ) -> Result<Option<CollectionOperationResponse>> {
        if self.has_collection(collection_name).await? {
            return Ok(None);
        }
        let response = self
            .create_collection(&CreateCollection {
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
        Ok(Some(response))
    }

    pub async fn upsert_knowledge(
        &self,
        collection_name: &str,
        knowledge: KnowledgePayload,
        embedding: Vec<f32>,
    ) -> Result<PointsOperationResponse> {
        let mut payload = Payload::new();
        trace!("Upserting knowledge: {:?}", &knowledge.title);
        payload.insert("title", knowledge.title);
        payload.insert("content", knowledge.content);
        payload.insert("url", knowledge.url);
        let point = PointStruct::new(Uuid::new_v4().to_string(), embedding, payload);
        self.upsert_points(collection_name, [point].to_vec(), None)
            .await
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

    match qdrant_client.create_knowledge_collection(collection).await {
        Ok(Some(response)) => info!("Creating collection operation response: {:?}", response),
        Ok(None) => info!("Collection {} already exists", collection),
        Err(why) => {
            error!("Collection {} creation failed: {:?}", collection, why);
            return Ok(());
        }
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
