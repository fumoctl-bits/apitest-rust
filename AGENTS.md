# AGENTS.md

API REST mínima para una app de notas. Guarda y devuelve notas con `GET` y `POST`, usando PostgreSQL (YugabyteDB YSQL) con columnas `JSONB`.

## Stack

| Componente | Detalle |
|---|---|
| Lenguaje | Rust (edition 2024) |
| Web | `axum` 0.8 + `tokio` |
| Base de datos | PostgreSQL-compatible (YugabyteDB en desarrollo) vía `sqlx` 0.8 (`postgres`, `json`, `chrono`) |
| Serialización | `serde`, `serde_json`, `chrono` |

## Estructura

```
Cargo.toml      # dependencias
src/main.rs     # todo el servidor: modelos, handlers, arranque y migración
AGENTS.md       # este archivo
```

## Cómo funciona

1. Al arrancar, [main.rs](src/main.rs) se conecta al servidor con `DATABASE_URL`.
2. Crea la base de datos **`notiz`** si no existe (`DB_NAME`). La conexión inicial solo sirve para esto.
3. Abre un pool (máx. 5 conexiones) sobre `notiz`.
4. Crea la tabla `notes` si no existe (`CREATE TABLE IF NOT EXISTS`, no hay sistema de migraciones).
5. Escucha en `127.0.0.1:3000`.

### Esquema

```sql
CREATE TABLE IF NOT EXISTS notes (
    id         BIGSERIAL PRIMARY KEY,
    data       JSONB NOT NULL,                  -- {"title", "content", "tags"}
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

El contenido de la nota (`title`, `content`, `tags`) vive completo en `data` (JSONB). `id` y `created_at` son columnas normales. En la respuesta, `data` se aplana (`#[serde(flatten)]`) junto a `id` y `created_at`.

### Modelos (en `src/main.rs`)

- `NoteData`: cuerpo de la nota. Se usa como request de `POST` y como contenido del JSONB.
- `NoteRow`: fila de la tabla (`FromRow`).
- `Note`: respuesta de la API (`id` + `NoteData` aplanado + `created_at`).

## Configuración

| Variable | Default | Descripción |
|---|---|---|
| `DATABASE_URL` | `postgres://yugabyte:yugabyte@localhost:5433/yugabyte` | Conexión al servidor. La base de la URL solo se usa para crear `notiz`; la app siempre trabaja sobre `notiz`. |

El servidor HTTP escucha en `127.0.0.1:3000` (fijo en el código).

## Especificación de la API

Base URL: `http://127.0.0.1:3000`. Todo en JSON (`Content-Type: application/json`). No hay autenticación.

### `GET /api/notes`

Lista todas las notas, de la más reciente a la más antigua (`ORDER BY created_at DESC, id DESC`).

- **200 OK**

```json
[
  {
    "id": 1,
    "title": "Compras",
    "content": "Leche y pan",
    "tags": ["casa"],
    "created_at": "2026-10-22T14:12:47.144226Z"
  }
]
```

Devuelve `[]` si no hay notas.

### `POST /api/notes`

Crea una nota.

**Body**

| Campo | Tipo | Requerido | Notas |
|---|---|---|---|
| `title` | string | sí | |
| `content` | string | sí | |
| `tags` | array de string | no | Por defecto `[]` |

```json
{ "title": "Compras", "content": "Leche y pan", "tags": ["casa"] }
```

**Respuestas**

- **201 Created**: devuelve la nota creada (mismo formato que los elementos de `GET`).
- **4xx**: error de axum al deserializar el body (JSON inválido, falta `title` o `content`, o falta el header `Content-Type`). Normalmente `415` o `422`.
- **500 Internal Server Error**: fallo de base de datos (el detalle se imprime en stderr, no en la respuesta).

### Ejemplos

```bash
curl -X POST localhost:3000/api/notes \
  -H 'Content-Type: application/json' \
  -d '{"title":"Compras","content":"Leche y pan","tags":["casa"]}'

curl localhost:3000/api/notes
```

## Desarrollo

```bash
cargo check          # compila
cargo run            # arranca (requiere la base accesible en DATABASE_URL)
```

### Base de datos local (YugabyteDB)

El contenedor se define en `~/Projects/ultix/hosts/containers.nix` (`oci-containers`, Podman, servicio `podman-yugabyte`, `autoStart = false`).

```bash
sudo systemctl start podman-yugabyte
podman exec yugabyte bin/yugabyted status    # esperar "YSQL Status: Ready" (1-2 min)
```

Puntos importantes de esa configuración:

- YSQL se publica en el puerto **5433** (no el 5432 de Postgres).
- Credenciales por defecto: usuario `yugabyte`, contraseña `yugabyte`.
- El contenedor usa `--advertise_address=127.0.0.1`. Sin esto, Yugabyte guarda en el volumen la IP DHCP del WiFi y falla al arrancar cuando esta cambia.
- También usa `--tserver_flags=pgsql_proxy_bind_address=0.0.0.0:5433`. Sin esto, YSQL solo escucha en el loopback del contenedor y el puerto publicado responde `Connection reset by peer`.
- Se puede usar un Postgres normal cambiando `DATABASE_URL`. El código solo usa SQL estándar de Postgres, `BIGSERIAL` y `JSONB`.

## Convenciones y notas para agentes

- Todo el código vive en `src/main.rs`; si crece, separar en módulos (`models`, `handlers`, `db`).
- Los errores de base de datos pasan por `db_error()`, que registra el error y responde `500`.
- Los comentarios del código están en español; mantener ese idioma.
- `CREATE DATABASE` no admite parámetros enlazados, por eso se interpola `DB_NAME`. Debe seguir siendo una constante, nunca entrada del usuario.
- Antes de cambiar rutas o el formato del JSON, actualizar la sección "Especificación de la API" de este archivo.
- Ideas no implementadas: `GET /api/notes/{id}`, `PUT`/`DELETE`, validación de campos vacíos, puerto configurable por variable de entorno.

