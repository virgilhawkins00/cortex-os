FROM python:3.11-slim

WORKDIR /app

# Install system dependencies needed by chromadb / sentence-transformers
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency manifest first (Docker layer cache)
COPY pyproject.toml .

# Install the package and all runtime dependencies
RUN pip install --no-cache-dir -e .

# Copy application source
COPY cortex_memory/ ./cortex_memory/

# Create data directory for SQLite
RUN mkdir -p /app/data

EXPOSE 8765

CMD ["python", "-m", "cortex_memory"]
