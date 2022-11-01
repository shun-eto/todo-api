mod handlers;
mod repositories;
use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use dotenv::dotenv;
use handlers::{
    label::{all_label, create_label, delete_label},
    todo::{all_todo, create_todo, delete_todo, find_todo, update_todo},
};
use repositories::{
    label::{LabelRepository, LabelRepositoryForDb},
    todo::{TodoRepository, TodoRepositoryForDb},
};
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() {
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");
    tracing::debug!("start connect database...");
    let pool = PgPool::connect(&database_url)
        .await
        .expect(&format!("fail connect database, url is [{}]", database_url));
    let todo_repository = TodoRepositoryForDb::new(pool.clone());
    let label_repository = LabelRepositoryForDb::new(pool.clone());
    let app = create_app(todo_repository, label_repository);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", "addr");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn create_app<TodoRepo: TodoRepository, LabelRepo: LabelRepository>(
    todo_repository: TodoRepo,
    label_repository: LabelRepo,
) -> Router {
    let app = Router::new()
        .route("/", get(root))
        .route(
            "/todos",
            post(create_todo::<TodoRepo>).get(all_todo::<TodoRepo>),
        )
        .route(
            "/todos/:id",
            get(find_todo::<TodoRepo>)
                .delete(delete_todo::<TodoRepo>)
                .patch(update_todo::<TodoRepo>),
        )
        .route(
            "/labels",
            post(create_label::<LabelRepo>).get(all_label::<LabelRepo>),
        )
        .route("/labels/:id", delete(delete_label::<LabelRepo>))
        .layer(Extension(Arc::new(todo_repository)))
        .layer(Extension(Arc::new(label_repository)));

    return app;
}

async fn root() -> &'static str {
    "Hello, World!"
}
