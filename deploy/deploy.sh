#!/bin/bash
set -e

SERVER="root@mira.local"
DEPLOY_DIR="/opt/chai"

echo "=== Building chai-rs for release ==="
cd "$(dirname "$0")/.."

# Clean old site files to force rebuild
rm -rf target/site/pkg

# Build release binary (from chai-web directory where Cargo.toml with leptos config is)
cd chai-web
# Touch source files to force frontend rebuild
touch src/lib.rs style/main.css
cargo leptos build --release
cd ..

echo ""
echo "=== Preparing deployment package ==="

# Verify build artifacts exist
if [ ! -f "target/site/pkg/chai-web.js" ]; then
    echo "ERROR: Frontend build failed - target/site/pkg/chai-web.js not found"
    exit 1
fi

# Create temp dir with deployment files
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

# Copy binary (built in workspace target/)
cp target/release/chai-web "$TEMP_DIR/"

# Copy site directory (CSS, JS, WASM)
cp -r target/site "$TEMP_DIR/site"

# Show what we're deploying
echo "Frontend files:"
ls -la target/site/pkg/

echo "Binary size: $(du -h target/release/chai-web | cut -f1)"
echo "Site size: $(du -sh target/site | cut -f1)"

echo ""
echo "=== Syncing to $SERVER ==="

# Create directories on server
ssh "$SERVER" "mkdir -p $DEPLOY_DIR/data"

# Sync files
rsync -avz --progress "$TEMP_DIR/chai-web" "$SERVER:$DEPLOY_DIR/"
rsync -avz --progress --delete "$TEMP_DIR/site/" "$SERVER:$DEPLOY_DIR/site/"

echo ""
echo "=== Installing systemd services ==="

# Copy and enable services
scp deploy/qdrant-chai.service deploy/chai.service "$SERVER:/etc/systemd/system/"
ssh "$SERVER" "systemctl daemon-reload && systemctl enable qdrant-chai chai"

echo ""
echo "=== Checking .env file ==="

# Check if .env exists
if ! ssh "$SERVER" "test -f $DEPLOY_DIR/.env"; then
    echo ""
    echo "WARNING: $DEPLOY_DIR/.env does not exist!"
    echo "Create it with:"
    echo ""
    echo "  ssh $SERVER \"cat > $DEPLOY_DIR/.env << EOF"
    echo "OPENROUTER_API_KEY=your-key-here"
    echo "JWT_SECRET=\$(openssl rand -base64 32)"
    echo "EOF\""
    echo ""
    echo "Then run: ssh $SERVER 'systemctl restart chai'"
    exit 1
fi

echo ""
echo "=== Restarting services ==="

ssh "$SERVER" "systemctl restart qdrant-chai && sleep 3 && systemctl restart chai"
ssh "$SERVER" "systemctl status chai --no-pager"

echo ""
echo "=== Deployment complete! ==="
echo ""
echo "Service: http://mira.local:3031"
echo "Qdrant:  http://mira.local:6333/dashboard"
echo ""
echo "Logs: ssh $SERVER 'journalctl -u chai -f'"
