use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    types::Json as SqlJson,
    Connection, FromRow, PgConnection, PgPool,
};
use std::str::FromStr;

// Nombre de la base de datos que usa la app
const DB_NAME: &str = "notiz";

// Contenido de la nota: se guarda completo en la columna JSONB `data`
#[derive(Serialize, Deserialize)]
struct NoteData {
    title: String,
    content: String,
    #[serde(default)]
    tags: Vec<String>,
}

// Fila de la tabla `notes`
#[derive(FromRow)]
struct NoteRow {
    id: i64,
    data: SqlJson<NoteData>,
    created_at: DateTime<Utc>,
}

// Respuesta de la API
#[derive(Serialize)]
struct Note {
    id: i64,
    #[serde(flatten)]
    data: NoteData,
    created_at: DateTime<Utc>,
}

impl From<NoteRow> for Note {
    fn from(row: NoteRow) -> Self {
        Note {
            id: row.id,
            data: row.data.0,
            created_at: row.created_at,
        }
    }
}

fn db_error(err: sqlx::Error) -> StatusCode {
    eprintln!("Database error: {err}");
    StatusCode::INTERNAL_SERVER_ERROR
}

// GET /api/notes -> lista todas las notas (más recientes primero)
async fn list_notes(State(pool): State<PgPool>) -> Result<Json<Vec<Note>>, StatusCode> {
    let rows = sqlx::query_as::<_, NoteRow>(
        "SELECT id, data, created_at FROM notes ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(db_error)?;

    Ok(Json(rows.into_iter().map(Note::from).collect()))
}

// POST /api/notes -> crea una nota (JSON: title, content, tags opcional)
async fn create_note(
    State(pool): State<PgPool>,
    Json(payload): Json<NoteData>,
) -> Result<(StatusCode, Json<Note>), StatusCode> {
    let row = sqlx::query_as::<_, NoteRow>(
        "INSERT INTO notes (data) VALUES ($1) RETURNING id, data, created_at",
    )
    .bind(SqlJson(&payload))
    .fetch_one(&pool)
    .await
    .map_err(db_error)?;

    Ok((StatusCode::CREATED, Json(row.into())))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Conectar al servidor (DATABASE_URL o valor por defecto local). Esta URL
    //    solo se usa para crear la base `notiz`; la app trabaja siempre sobre `notiz`.
    let server_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://yugabyte:yugabyte@localhost:5433/yugabyte".to_string());
    let server_opts = PgConnectOptions::from_str(&server_url)?;

    // 2. Crear la base de datos `notiz` si no existe
    let mut admin = PgConnection::connect_with(&server_opts).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(DB_NAME)
        .fetch_one(&mut admin)
        .await?;
    if !exists {
        // CREATE DATABASE no admite parámetros; DB_NAME es una constante
        sqlx::query(&format!("CREATE DATABASE \"{DB_NAME}\""))
            .execute(&mut admin)
            .await?;
        println!("Base de datos '{DB_NAME}' creada");
    }
    admin.close().await?;

    // 3. Conectar a la base `notiz`
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(server_opts.database(DB_NAME))
        .await?;

    // 4. Crear la tabla de notas si no existe (contenido en JSONB)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS notes (
            id BIGSERIAL PRIMARY KEY,
            data JSONB NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        "#,
    )
    .execute(&pool)
    .await?;

    // 5. Registrar rutas y compartir el pool con with_state()
    let app = Router::new()
        .route("/api/notes", get(list_notes).post(create_note))
        .with_state(pool);

    // 6. Iniciar el servidor
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await?;

    Ok(())
}