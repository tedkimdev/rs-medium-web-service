use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use sqlx::types::chrono;
use uuid::Uuid;

pub mod postgres_queue;
pub mod error;
pub use error::Error;

#[async_trait::async_trait]
pub trait Queue: Send + Sync + Debug {
    async fn push(
        &self,
        job: Message,
        date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), crate::Error>;
    /// pull fetches at most `number_of_jobs` from the queue.
    async fn pull(&self, number_of_jobs: u32) -> Result<Vec<Job>, crate::Error>;
    async fn delete_job(&self, job_id: Uuid) -> Result<(), crate::Error>;
    async fn fail_job(&self, job_id: Uuid) -> Result<(), crate::Error>;
    async fn clear(&self) -> Result<(), crate::Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub message: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    SendSignInEmail {
        email: String,
        name: String,
        code: String,
    },
    DeleteOldUserData,
    SendNewsletterMessage {
        message_id: Uuid,
    },
    // ...
}
