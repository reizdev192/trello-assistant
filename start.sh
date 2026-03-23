#!/usr/bin/env bash
set -e

echo "🚀 Trello Assistant — One-Click Start"
echo "======================================="

# Build frontend if dist/ is missing or frontend source changed
if [ ! -d "dist" ] || [ "frontend/src" -nt "dist/index.html" ] 2>/dev/null; then
    echo ""
    echo "📦 Building frontend..."
    cd frontend
    npm install --silent 2>/dev/null
    npm run build
    cd ..
    echo "✅ Frontend built successfully"
else
    echo "✅ Frontend already built (dist/ exists)"
fi

echo ""
echo "🦀 Starting Rust server..."
echo ""

cargo run
