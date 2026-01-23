# Deployment to mira.local

## Prerequisites

- Rust toolchain with `cargo-leptos` installed
- SSH access to `root@mira.local`
- Docker installed on mira.local

## Quick Deploy

```bash
./deploy/deploy.sh
```

## Manual Steps

### 1. First-time setup: Create .env file

```bash
ssh root@mira.local "cat > /opt/chai/.env << 'EOF'
OPENROUTER_API_KEY=sk-or-v1-your-key-here
JWT_SECRET=$(openssl rand -base64 32)
EOF"
```

### 2. Sync tea data to Qdrant

After deployment, sync the tea database:

```bash
# On your local machine (with cached HTML)
cargo run --package chai-cli -- sync --from-cache
```

Or copy the local Qdrant data:

```bash
# Stop local Qdrant first for data integrity
docker stop <local-qdrant-container>
rsync -avz data/qdrant_storage/ root@mira.local:/opt/qdrant_storage/
docker start <local-qdrant-container>
ssh root@mira.local "systemctl restart qdrant-chai"
```

### 3. Configure reverse proxy (cloud-forge)

Nginx config already added to cloud-forge:
- `roles/nginx/templates/chai.conf.j2`
- `group_vars/moscow/vars.yml` (chai: 3031, chai.okhsunrog.ru)

To deploy (SSL certificate obtained automatically):

```bash
cd ~/code/cloud-forge
ansible-playbook site-moscow.yml
```

## Architecture

```
Internet
    |
    v (port 443)
Moscow VPS (HAProxy + Nginx)
    |
    v (WireGuard 10.66.66.x)
mira.local
    |
    +-- qdrant-chai.service (Docker, ports 6333/6334)
    |       +-- /opt/qdrant_storage/
    |
    +-- chai.service (port 3031, depends on qdrant-chai)
            +-- Leptos SSR (Axum)
            +-- SQLite: /opt/chai/data/chai.db
```

## Service Management

```bash
# View chai logs
ssh root@mira.local "journalctl -u chai -f"

# View Qdrant logs
ssh root@mira.local "journalctl -u qdrant-chai -f"

# Restart services
ssh root@mira.local "systemctl restart qdrant-chai chai"

# Qdrant dashboard (via SSH tunnel)
ssh -L 6333:127.0.0.1:6333 root@mira.local
# Then open http://localhost:6333/dashboard
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENROUTER_API_KEY` | OpenRouter API key for embeddings/LLM | (required) |
| `JWT_SECRET` | Secret for JWT token signing | (required) |
| `LEPTOS_SITE_ADDR` | Server bind address | `0.0.0.0:3031` |
| `SQLITE_DATABASE_PATH` | SQLite database path | `/opt/chai/data/chai.db` |
| `QDRANT_URL` | Qdrant gRPC URL | `http://127.0.0.1:6334` |
| `QDRANT_COLLECTION` | Qdrant collection name | `teas` |
| `VECTOR_SIZE` | Embedding vector size | `4096` |

## Data Locations

| Data | Path |
|------|------|
| Binary | `/opt/chai/chai-web` |
| Static assets | `/opt/chai/site/` |
| SQLite DB | `/opt/chai/data/chai.db` |
| Qdrant storage | `/opt/qdrant_storage/` |
| Environment | `/opt/chai/.env` |
| Systemd services | `/etc/systemd/system/chai.service`, `/etc/systemd/system/qdrant-chai.service` |
