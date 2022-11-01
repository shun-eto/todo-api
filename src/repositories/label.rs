use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::RepositoryError;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Label {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateLabel {
    id: i32,
    name: String,
}

#[async_trait]
pub trait LabelRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, name: String) -> anyhow::Result<Label>;
    async fn all(&self) -> anyhow::Result<Vec<Label>>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct LabelRepositoryForDb {
    pool: PgPool,
}

impl LabelRepositoryForDb {
    pub fn new(pool: PgPool) -> Self {
        return Self { pool };
    }
}

#[async_trait]
impl LabelRepository for LabelRepositoryForDb {
    async fn create(&self, name: String) -> anyhow::Result<Label> {
        let optional_label = sqlx::query_as::<_, Label>(
            r#"
        select * from labels where name = $1
        "#,
        )
        .bind(name.clone())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(label) = optional_label {
            return Err(RepositoryError::Duplicate(label.id).into());
        }

        let label = sqlx::query_as::<_, Label>(
            r#"
        insert into labels (name)
        values ($1)
        returning *
        "#,
        )
        .bind(name.clone())
        .fetch_one(&self.pool)
        .await?;

        return Ok(label);
    }

    async fn all(&self) -> anyhow::Result<Vec<Label>> {
        let labels = sqlx::query_as::<_, Label>(
            r#"
            select * from labels
            order by labels.id asc;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        return Ok(labels);
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        delete from labels where id=$1
        "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::UnexpectedError(e.to_string()),
        })?;

        return Ok(());
    }
}

#[cfg(test)]
mod test {
    use std::env;

    use dotenv::dotenv;

    use super::*;

    #[tokio::test]
    async fn crud_scenario() {
        dotenv().ok();
        let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");
        let pool = PgPool::connect(database_url)
            .await
            .expect(&format!("fail connect database, url is [{}]", database_url));

        let repository = LabelRepositoryForDb::new(pool);
        let label_text = "test_label";

        //  create
        let label = repository
            .create(label_text.to_string())
            .await
            .expect("[create] returned Error");
        assert_eq!(label.name, label_text);

        //  all
        let labels = repository.all().await.expect("[all] returned Error");
        let label = labels.last().unwrap();
        assert_eq!(label.name, label_text);

        //  delete
        repository
            .delete(label.id)
            .await
            .expect("[delete] returned Error");
    }
}
