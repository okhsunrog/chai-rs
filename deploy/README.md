# Deployment to mira.local

## Prerequisites

- Rust toolchain with `cargo-leptos` installed
- SSH access to `root@mira.local`

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

### 2. Sync tea data to database

After deployment, sync the tea database:

```bash
# On your local machine (with cached HTML)
cargo run --package chai-cli -- sync --from-cache
```

Or copy the local database file:

```bash
rsync -avz data/chai.db root@mira.local:/opt/chai/data/
ssh root@mira.local "systemctl restart chai"
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
    +-- chai.service (port 3031)
            +-- Leptos SSR (Axum)
            +-- Turso DB: /opt/chai/data/chai.db
                (users, cache, teas with vector embeddings)
```

## Service Management

```bash
# View chai logs
ssh root@mira.local "journalctl -u chai -f"

# Restart service
ssh root@mira.local "systemctl restart chai"

# Check service status
ssh root@mira.local "systemctl status chai"
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENROUTER_API_KEY` | OpenRouter API key for embeddings/LLM | (required) |
| `JWT_SECRET` | Secret for JWT token signing | (required) |
| `LEPTOS_SITE_ADDR` | Server bind address | `0.0.0.0:3031` |
| `DATABASE_PATH` | Turso database path | `/opt/chai/data/chai.db` |
| `VECTOR_SIZE` | Embedding vector size | `4096` |

## Data Locations

| Data | Path |
|------|------|
| Binary | `/opt/chai/chai-web` |
| Static assets | `/opt/chai/site/` |
| Database | `/opt/chai/data/chai.db` |
| Environment | `/opt/chai/.env` |
| Systemd service | `/etc/systemd/system/chai.service` |
