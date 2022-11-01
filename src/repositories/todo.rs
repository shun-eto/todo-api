use anyhow::Context;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use validator::Validate;

use super::RepositoryError;

#[async_trait]
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<Todo>;
    async fn find(&self, id: i32) -> anyhow::Result<Todo>;
    async fn all(&self) -> anyhow::Result<Vec<Todo>>;
    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, FromRow)]
pub struct Todo {
    id: i32,
    text: String,
    completed: bool,
}

impl Todo {
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

type TodoDatas = HashMap<i32, Todo>;

#[derive(Debug, Clone)]
pub struct TodoRepositoryForMemory {
    store: Arc<RwLock<TodoDatas>>,
}
impl TodoRepositoryForMemory {
    pub fn new() -> Self {
        TodoRepositoryForMemory {
            store: Arc::default(),
        }
    }

    fn write_store_ref(&self) -> RwLockWriteGuard<TodoDatas> {
        return self.store.write().unwrap();
    }

    fn read_store_ref(&self) -> RwLockReadGuard<TodoDatas> {
        return self.store.read().unwrap();
    }
}

#[async_trait]
impl TodoRepository for TodoRepositoryForMemory {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<Todo> {
        let mut store = self.write_store_ref();
        let id = (store.len() + 1) as i32;
        let todo = Todo::new(id, payload.text.clone());
        store.insert(id, todo.clone());
        return Ok(todo);
    }

    async fn find(&self, id: i32) -> anyhow::Result<Todo> {
        let store = self.read_store_ref();
        let todo = store
            .get(&id)
            .map(|todo| todo.clone())
            .ok_or(RepositoryError::NotFound(id))?;
        return Ok(todo);
    }

    async fn all(&self) -> anyhow::Result<Vec<Todo>> {
        let store = &self.read_store_ref();
        let todos = Vec::from_iter(store.values().map(|todo| todo.clone()));
        return Ok(todos);
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo> {
        let mut store = self.write_store_ref();
        let todo = store.get(&id).context(RepositoryError::NotFound(id))?;
        let text = payload.text.unwrap_or(todo.text.clone());
        let completed = payload.completed.unwrap_or(todo.completed);
        let todo = Todo {
            id,
            text,
            completed,
        };

        store.insert(id, todo.clone());
        return Ok(todo);
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let mut store = self.write_store_ref();
        store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
        return Ok(());
    }
}

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
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<Todo> {
        let todo = sqlx::query_as::<_, Todo>(
            r#"
                insert into todos (text, completed)
                values ($1, false)
                returning *
            "#,
        )
        .bind(payload.text.clone())
        .fetch_one(&self.pool)
        .await?;

        return Ok(todo);
    }

    async fn find(&self, id: i32) -> anyhow::Result<Todo> {
        let todo = sqlx::query_as::<_, Todo>(
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

        return Ok(todo);
    }

    async fn all(&self) -> anyhow::Result<Vec<Todo>> {
        let todos = sqlx::query_as::<_, Todo>(
            r#"
                select * from todos
                order by id desc;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        return Ok(todos);
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo> {
        let old_todo = self.find(id).await?;
        let todo = sqlx::query_as::<_, Todo>(
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

        return Ok(todo);
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
    async fn todo_crud_scenario() {
        let text = "todo text".to_string();
        let id = 1;
        let expected = Todo::new(id, text.clone());

        //  create
        let repository = TodoRepositoryForMemory::new();
        let todo = repository
            .create(CreateTodo { text })
            .await
            .expect("failed create todo");
        assert_eq!(expected, todo);

        //  find
        let todo = repository.find(todo.id).await.unwrap();
        assert_eq!(expected, todo);

        //  all
        let todos = repository.all().await.expect("failed get all todo");
        assert_eq!(vec![expected], todos);

        //  update
        let text = "update todo text".to_string();
        let todo = repository
            .update(
                id,
                UpdateTodo {
                    text: Some(text.clone()),
                    completed: Some(true),
                },
            )
            .await
            .expect("failed update todo.");
        assert_eq!(
            Todo {
                id,
                text,
                completed: true
            },
            todo
        );

        //  delete
        let res = repository.delete(id).await;
        assert!(res.is_ok());
    }

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
