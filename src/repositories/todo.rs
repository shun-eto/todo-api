use anyhow::Context;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use validator::Validate;

use super::{label::Label, RepositoryError};

#[async_trait]
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity>;
    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity>;
    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>>;
    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, FromRow)]
pub struct TodoWithLabelFromRow {
    id: i32,
    text: String,
    completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TodoEntity {
    pub id: i32,
    pub text: String,
    pub completed: bool,
    pub labels: Vec<Label>,
}

fn fold_entities(rows: Vec<TodoWithLabelFromRow>) -> Vec<TodoEntity> {
    rows.into_iter()
        .fold(vec![], |mut accum: Vec<TodoEntity>, current| {
            accum.push(TodoEntity {
                id: current.id,
                text: current.text.clone(),
                completed: current.completed,
                labels: vec![],
            });

            return accum;
        })
}

fn fold_entity(row: TodoWithLabelFromRow) -> TodoEntity {
    let todo_entities = fold_entities(vec![row]);
    let todo = todo_entities.first().expect("expect 1 todo");

    return todo.clone();
}

impl TodoWithLabelFromRow {
    pub fn new(id: i32, text: String) -> Self {
        Self {
            id,
            text,
            completed: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct CreateTodo {
    #[validate(length(min = 1, message = "can not be empty"))]
    text: String,
}

#[cfg(test)]
impl CreateTodo {
    pub fn new(text: String) -> Self {
        return Self { text };
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateTodo {
    text: Option<String>,
    completed: Option<bool>,
}

type TodoDatas = HashMap<i32, TodoWithLabelFromRow>;

#[derive(Debug, Clone)]
pub struct TodoRepositoryForDb {
    pool: PgPool,
}

impl TodoRepositoryForDb {
    pub fn new(pool: PgPool) -> Self {
        return Self { pool };
    }
}

#[async_trait]
impl TodoRepository for TodoRepositoryForDb {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity> {
        let todo = sqlx::query_as::<_, TodoWithLabelFromRow>(
            r#"
                insert into todos (text, completed)
                values ($1, false)
                returning *
            "#,
        )
        .bind(payload.text.clone())
        .fetch_one(&self.pool)
        .await?;

        return Ok(fold_entity(todo));
    }

    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
        let todo = sqlx::query_as::<_, TodoWithLabelFromRow>(
            r#"
                select * from todos where id=$1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::UnexpectedError(e.to_string()),
        })?;

        return Ok(fold_entity(todo));
    }

    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
        let todos = sqlx::query_as::<_, TodoWithLabelFromRow>(
            r#"
                select * from todos
                order by id desc;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        return Ok(fold_entities(todos));
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity> {
        let old_todo = self.find(id).await?;
        let todo = sqlx::query_as::<_, TodoWithLabelFromRow>(
            r#"
                update todos set text=$1, completed=$2
                where id=$3
                returning *
            "#,
        )
        .bind(payload.text.unwrap_or(old_todo.text))
        .bind(payload.completed.unwrap_or(old_todo.completed))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        return Ok(fold_entity(todo));
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        sqlx::query(
            r#"
                delete from todos where id=$1
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
        let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_UR+]");
        let pool = PgPool::connect(database_url)
            .await
            .expect(&format!("fail connect database, url is [{}]", database_url));
        let repository = TodoRepositoryForDb::new(pool.clone());
        let todo_text = "[crud_scenario] text";

        //  create
        let created = repository
            .create(CreateTodo::new(todo_text.to_string()))
            .await
            .expect("[create] return Err");
        assert_eq!(created.text, todo_text);
        assert!(!created.completed);

        //  find
        let todo = repository
            .find(created.id)
            .await
            .expect("[find] returned Err");
        assert_eq!(created, todo);

        //  all
        let todos = repository.all().await.expect("[all] returned Err");
        let todo = todos.first().unwrap();
        assert_eq!(created, *todo);

        //  update
        let updated_text = "[curd_scenario] updated text";
        let todo = repository
            .update(
                todo.id,
                UpdateTodo {
                    text: Some(updated_text.to_string()),
                    completed: Some(true),
                },
            )
            .await
            .expect("[update] returned Err");
        assert_eq!(created.id, todo.id);

        //  delete
        let _ = repository
            .delete(todo.id)
            .await
            .expect("[delete] returned Err");
        let res = repository.find(created.id).await;
        assert!(res.is_err());

        let todo_rows = sqlx::query(
            r#"
                select * from todos where id=$1
            "#,
        )
        .bind(todo.id)
        .fetch_all(&pool)
        .await
        .expect("[delete] todo_labels fetch error");
        assert!(todo_rows.len() == 0);
    }
}
