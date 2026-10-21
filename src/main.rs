use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, SqlitePool};

// Struct matching our database table
#[derive(Serialize, FromRow)]
struct Item {
    id: i64,
    name: String,
}

// Request payload for creating an item
#[derive(Deserialize)]
struct CreateItemRequest {
    name: String,
}

// GET /api/items -> Query all items from SQLite
async fn list_items(State(pool): State<SqlitePool>) -> Result<Json<Vec<Item>>, StatusCode> {
    let items = sqlx::query_as::<_, Item>("SELECT id, name FROM items")
        .fetch_all(&pool)
        .await
        .map_err(|err| {
            eprintln!("Database error: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(items))
}

// POST /api/items -> Insert an item into SQLite
async fn create_item(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateItemRequest>,
) -> Result<(StatusCode, Json<Item>), StatusCode> {
    let result = sqlx::query("INSERT INTO items (name) VALUES (?)")
        .bind(&payload.name)
        .execute(&pool)
        .await
        .map_err(|err| {
            eprintln!("Insert failed: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let inserted_id = result.last_insert_rowid();

    let item = Item {
        id: inserted_id,
        name: payload.name,
    };

    Ok((StatusCode::CREATED, Json(item)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to SQLite (creates items.db if it doesn't exist)
    let db_url = "sqlite://items.db?mode=rwc";
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    // 2. Create the items table if it doesn't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    // 3. Register routes and share the connection pool via with_state()
    let app = Router::new()
        .route("/api/items", get(list_items).post(create_item))
        .with_state(pool);

    // 4. Start the listener
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await?;

    Ok(())
}