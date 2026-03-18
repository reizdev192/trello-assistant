#!/bin/sh
# Wait for Ollama server to be ready, then pull the model
set -e

echo "⏳ Waiting for Ollama server..."
until curl -sf http://localhost:11434/api/tags > /dev/null 2>&1; do
  sleep 2
done

echo "✅ Ollama server is ready!"

MODEL="${OLLAMA_MODEL:-qwen3.5:0.8b}"
echo "📦 Pulling model: $MODEL"
ollama pull "$MODEL"
echo "✅ Model $MODEL is ready!"

# Keep container alive
tail -f /dev/null
