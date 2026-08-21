#!/bin/bash
set -e

mkdir -p data
docker stop crypto-market-collector 2>/dev/null || true
docker rm crypto-market-collector 2>/dev/null || true

docker build -t market-collector .
docker run -d --name crypto-market-collector --restart unless-stopped -v $(pwd)/data:/app/data market-collector

echo "✅ Container started successfully! Streaming logs..."
docker logs -f crypto-market-collector
