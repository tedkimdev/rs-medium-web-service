use sqlx::types::{chrono, Json, Uuid};
use sqlx::PgPool;
use ulid::Ulid;

use crate::{Job, Message, Queue};

const MAX_FAILED_ATTEMPTS: i32 = 3; // low, as most jobs also use retries internally


#[derive(sqlx::FromRow, Debug, Clone)]
struct PostgresJob {
    id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,

    scheduled_for: chrono::DateTime<chrono::Utc>,
    failed_attempts: i32,
    status: PostgresJobStatus,
    message: Json<Message>,
}

// We use a INT as Postgres representation for performance reasons
#[derive(Debug, Clone, sqlx::Type, PartialEq)]
#[repr(i32)]
enum PostgresJobStatus {
    Queued,
    Running,
    Failed,
}

impl From<PostgresJob> for Job {
    fn from(item: PostgresJob) -> Self {
        Job {
            id: item.id,
            message: item.message.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostgresQueue {
    db: PgPool,
    max_attempts: u32,
}

impl PostgresQueue {
    pub fn new(db: PgPool) -> PostgresQueue {
        PostgresQueue {
            db,
            max_attempts: 5,
        }
    }
}

#[async_trait::async_trait]
impl Queue for PostgresQueue {
    async fn push(
        &self,
        job: Message,
        date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), crate::Error> {
        let now = chrono::Utc::now();
        let scheduled_for = date.unwrap_or(now);
        let failed_attempts = 0;
        let message = Json(job);
        let status = PostgresJobStatus::Queued;
        let job_id: Uuid = Ulid::new().into();
        let query = "INSERT INTO queue
            (id, created_at, updated_at, scheduled_for, failed_attempts, status, message)
            VALUES ($1, $2, $3, $4, $5, $6, $7)";

        sqlx::query(query)
            .bind(job_id)
            .bind(now)
            .bind(now)
            .bind(scheduled_for)
            .bind(failed_attempts)
            .bind(status)
            .bind(message)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn pull(&self, number_of_jobs: u32) -> Result<Vec<Job>, crate::Error> {
        let now = chrono::Utc::now();
        let query = "Update queue
            SET status = $1, updated_at = $2
            WHERE id IN (
                SELECT id
                    FROM queue
                WHERE status = $3
                    AND scheduled_for <= $4
                    AND failed_attempts < $5
                ORDER BY scheduled_for
                FOR UPDATE SKIP LOCKED
                LIMIT $6
            )
            RETURNING *
        ";

        let jobs: Vec<PostgresJob> = sqlx::query_as::<_, PostgresJob>(query)
            .bind(PostgresJobStatus::Running)
            .bind(now)
            .bind(PostgresJobStatus::Queued)
            .bind(now)
            .bind(MAX_FAILED_ATTEMPTS)
            .bind(number_of_jobs as i32)
            .fetch_all(&self.db)
            .await?;

        Ok(jobs.into_iter().map(Into::into).collect())
    }

    async fn delete_job(&self, job_id: Uuid) -> Result<(), crate::Error> {
        let query = "DELETE FROM queue WHERE id = $1";

        sqlx::query(query).bind(job_id).execute(&self.db).await?;
        Ok(())
    }

    async fn fail_job(&self, job_id: Uuid) -> Result<(), crate::Error> {
        let now = chrono::Utc::now();
        let query = "UPDATE queue
            SET status = $1, updated_at = $2, failed_attempts = failed_attempts + 1
            WHERE id = $3";

        sqlx::query(query)
            .bind(PostgresJobStatus::Queued)
            .bind(now)
            .bind(job_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn clear(&self) -> Result<(), crate::Error> {
        let query = "DELETE FROM queue";

        sqlx::query(query).execute(&self.db).await?;
        Ok(())
    }
}
