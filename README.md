# Tea Advisor

AI-powered tea discovery platform with semantic search. Scrapes tea data from [beliyles.com](https://beliyles.com), creates vector embeddings, and provides personalized recommendations through a web UI.

## Features

- **Semantic Search** - Find teas by description, taste, mood, or ingredients using vector similarity
- **Two-Stage AI Pipeline** - Query analysis + intelligent selection from candidates
- **Smart Filters** - Exclude samples, sets, out-of-stock items; filter by series
- **User Authentication** - JWT-based auth with Argon2 password hashing
- **Modern Stack** - Leptos 0.8 (Rust WASM), Axum, Turso (embedded SQLite with vectors)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  User Query                                             │
└─────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 1: Query Analysis (LLM)                         │
│  → Extracts: search terms, count, filters              │
└─────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 2: Vector Search (Turso)                        │
│  → Returns N+4 candidates via cosine similarity        │
└─────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 3: Selection & Description (LLM)                │
│  → Picks best N teas, generates descriptions           │
└─────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Rust 2024 edition
- [cargo-leptos](https://github.com/leptos-rs/cargo-leptos)
- OpenRouter API key

### Setup

1. Clone and configure:
```bash
git clone https://github.com/okhsunrog/chai-rs
cd chai-rs
cp .env.example .env
# Edit .env with your OPENROUTER_API_KEY and JWT_SECRET
```

2. Populate the database:
```bash
# Cache tea pages from website
cargo run --package chai-cli -- cache

# Create embeddings and sync to database
cargo run --package chai-cli -- sync --from-cache
```

3. Run the web app:
```bash
cd chai-web
cargo leptos watch
```

Open http://localhost:3000

## CLI Commands

```bash
# Cache HTML pages from website
cargo run --package chai-cli -- cache

# Sync teas to database with embeddings
cargo run --package chai-cli -- sync --from-cache [--force]

# Search teas
cargo run --package chai-cli -- search "spicy warming tea" --limit 5

# Show database statistics
cargo run --package chai-cli -- stats
```

## Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENROUTER_API_KEY` | OpenRouter API key | (required) |
| `JWT_SECRET` | JWT signing secret | (required) |
| `DATABASE_PATH` | Turso database path | `data/chai.db` |
| `EMBEDDINGS_MODEL` | Embedding model | `qwen/qwen3-embedding-8b` |
| `VECTOR_SIZE` | Embedding dimensions | `4096` |

## Project Structure

```
chai-rs/
├── chai-core/          # Shared library (AI, auth, database, scraper)
├── chai-cli/           # CLI tool for data management
├── chai-web/           # Web UI (Leptos + Axum)
├── deploy/             # Deployment scripts and systemd services
└── data/
    └── chai.db         # Turso database (users, cache, teas + embeddings)
```

## Tech Stack

- **Frontend**: Leptos 0.8 (Rust → WASM), SSR + Hydration
- **Backend**: Axum, Tower (rate limiting)
- **Database**: Turso (libSQL - SQLite with vector search)
- **AI**: OpenRouter (embeddings + LLM)
- **Auth**: JWT + Argon2

## License

MIT
