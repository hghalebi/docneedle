#!/usr/bin/env bash

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log() {
  printf '[%s] %s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" "$*"
}

load_environment_file() {
  local env_file="${PDF_SEARCH_ENV_FILE:-${PROJECT_ROOT}/.env}"

  if [[ -f "$env_file" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$env_file"
    set +a
    echo "loaded environment from ${env_file}"
  fi
}

load_environment_file

COMPOSE_FILE="${PDF_SEARCH_COMPOSE_FILE:-${PROJECT_ROOT}/deploy/docker-compose.yml}"
AUTO_START_STACK="${AUTO_START_STACK:-true}"
SETUP_TIMEOUT_SECONDS="${SETUP_TIMEOUT_SECONDS:-360}"

OPENSEARCH_URL="${OPENSEARCH_URL:-http://localhost:9200}"
QDRANT_URL="${QDRANT_URL:-http://localhost:6333}"
NEO4J_URL="${NEO4J_URL:-http://localhost:7474}"
NEO4J_DB="${NEO4J_DB:-neo4j}"
NEO4J_USER="${NEO4J_USER:-neo4j}"
NEO4J_PASSWORD="${NEO4J_PASSWORD:-password}"
NEO4J_AUTH="${NEO4J_AUTH:-${NEO4J_USER}/${NEO4J_PASSWORD}}"

OPENSEARCH_INDEX="${OPENSEARCH_INDEX:-pdf_chunks}"
QDRANT_COLLECTION="${QDRANT_COLLECTION:-pdf_chunks}"
EMBEDDING_DIMENSIONS="${EMBEDDING_DIMENSIONS:-128}"

WAIT_SECONDS=2

compose_binary() {
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    echo "docker compose"
    return
  fi

  if command -v docker-compose >/dev/null 2>&1; then
    echo "docker-compose"
    return
  fi

  log "docker compose is required when AUTO_START_STACK=true"
  exit 1
}

run_compose() {
  local compose_cmd
  compose_cmd="$(compose_binary)"

  if [[ "${compose_cmd}" == "docker compose" ]]; then
    docker compose -f "${COMPOSE_FILE}" "$@"
  else
    docker-compose -f "${COMPOSE_FILE}" "$@"
  fi
}

http_status() {
  local url="$1"
  shift || true

  curl -sS -o /tmp/last_http_body.json -w "%{http_code}" "$@" "$url" || true
}

wait_for_http_service() {
  local name="$1"
  local url="$2"
  shift 2
  local status=""
  local elapsed=0

  log "waiting for ${name}"
  while [[ "$elapsed" -lt "$SETUP_TIMEOUT_SECONDS" ]]; do
    status=$(http_status "$url" "$@")
    if [[ "$status" == "200" ]]; then
      log "${name} is ready"
      return 0
    fi
    sleep "$WAIT_SECONDS"
    elapsed=$((elapsed + WAIT_SECONDS))
  done

  log "${name} readiness check timed out (status: ${status})"
  if [[ -f /tmp/last_http_body.json ]]; then
    cat /tmp/last_http_body.json
  fi
  if [[ "${AUTO_START_STACK}" == "true" ]]; then
    log "diagnostic: showing service status and last logs"
    run_compose ps
    log "diagnostic: last opensearch logs"
    run_compose logs --no-color --tail=120 opensearch
    log "diagnostic: last qdrant logs"
    run_compose logs --no-color --tail=120 qdrant
    log "diagnostic: last neo4j logs"
    run_compose logs --no-color --tail=120 neo4j
  fi
  exit 1
}

run_neo4j_statement() {
  local statement="$1"
  local escaped_statement="${statement//\"/\\\"}"
  local payload
  payload="{\"statements\":[{\"statement\":\"${escaped_statement}\"}]}"

  local status
  status=$(curl -sS -o /tmp/last_http_body.json -w "%{http_code}" \
    -u "${NEO4J_USER}:${NEO4J_PASSWORD}" \
    -H 'Content-Type: application/json' \
    -X POST \
    "${NEO4J_URL}/db/${NEO4J_DB}/tx/commit" \
    -d "$payload" || true)

  if [[ "$status" != "200" ]]; then
    log "Neo4j statement failed: ${statement}"
    log "http status: ${status}"
    cat /tmp/last_http_body.json 2>/dev/null || true
    exit 1
  fi
}

ensure_opensearch_index() {
  local status
  status=$(http_status "${OPENSEARCH_URL}/${OPENSEARCH_INDEX}")
  if [[ "$status" == "200" ]]; then
    log "OpenSearch index ${OPENSEARCH_INDEX} already exists"
    return
  fi

  if [[ "$status" != "404" ]]; then
    log "could not determine OpenSearch index state (status=${status})"
    exit 1
  fi

  log "creating OpenSearch index ${OPENSEARCH_INDEX}"
  cat > /tmp/opensearch_index_payload.json <<EOF
{
  "settings": {
    "number_of_shards": 1,
    "number_of_replicas": 0,
    "analysis": {
      "analyzer": {
        "standard_english": {
          "type": "standard"
        }
      }
    }
  },
  "mappings": {
    "properties": {
      "text_raw": { "type": "text", "analyzer": "standard_english" },
      "text_normalized": { "type": "text", "analyzer": "standard_english" },
      "section_path": { "type": "keyword" },
      "document_id": { "type": "keyword" },
      "source_path": { "type": "keyword" },
      "clause_id": { "type": "keyword" },
      "standard": { "type": "keyword" },
      "version": { "type": "keyword" },
      "page_start": { "type": "integer" },
      "page_end": { "type": "integer" },
      "chunk_index": { "type": "long" }
    }
  }
}
EOF

  status=$(curl -sS -o /tmp/last_http_body.json -w "%{http_code}" \
    -X PUT \
    -H 'Content-Type: application/json' \
    "${OPENSEARCH_URL}/${OPENSEARCH_INDEX}" \
    --data-binary @/tmp/opensearch_index_payload.json || true)

  if [[ "$status" != "200" && "$status" != "201" ]]; then
    log "failed creating OpenSearch index (status=${status})"
    cat /tmp/last_http_body.json 2>/dev/null || true
    exit 1
  fi

  log "created OpenSearch index ${OPENSEARCH_INDEX}"
}

ensure_qdrant_collection() {
  local status
  status=$(http_status "${QDRANT_URL}/collections/${QDRANT_COLLECTION}")
  if [[ "$status" == "200" ]]; then
    if ! grep -q "\"size\"[[:space:]]*:[[:space:]]*${EMBEDDING_DIMENSIONS}" /tmp/last_http_body.json; then
      log "Qdrant collection ${QDRANT_COLLECTION} already exists but has unexpected vector size"
      cat /tmp/last_http_body.json
      exit 1
    fi

    log "Qdrant collection ${QDRANT_COLLECTION} already exists"
    return
  fi

  if [[ "$status" != "404" ]]; then
    log "could not determine Qdrant collection state (status=${status})"
    cat /tmp/last_http_body.json 2>/dev/null || true
    exit 1
  fi

  log "creating Qdrant collection ${QDRANT_COLLECTION}"
  cat > /tmp/qdrant_collection_payload.json <<EOF
{
  "vectors": {
    "size": ${EMBEDDING_DIMENSIONS},
    "distance": "Cosine"
  }
}
EOF

  status=$(curl -sS -o /tmp/last_http_body.json -w "%{http_code}" \
    -X PUT \
    -H 'Content-Type: application/json' \
    "${QDRANT_URL}/collections/${QDRANT_COLLECTION}" \
    --data-binary @/tmp/qdrant_collection_payload.json || true)

  if [[ "$status" != "200" ]]; then
    log "failed creating Qdrant collection (status=${status})"
    cat /tmp/last_http_body.json 2>/dev/null || true
    exit 1
  fi

  log "created Qdrant collection ${QDRANT_COLLECTION}"
}

ensure_neo4j_schema() {
  log "ensuring Neo4j constraints and indexes"
  run_neo4j_statement "CREATE CONSTRAINT IF NOT EXISTS FOR (d:Document) REQUIRE d.document_id IS UNIQUE"
  run_neo4j_statement "CREATE CONSTRAINT IF NOT EXISTS FOR (c:Chunk) REQUIRE c.chunk_id IS UNIQUE"
  run_neo4j_statement "CREATE INDEX IF NOT EXISTS FOR (c:Chunk) ON (c.document_id)"
  run_neo4j_statement "CREATE INDEX IF NOT EXISTS FOR (c:Chunk) ON (c.section_path)"
  run_neo4j_statement "CREATE INDEX IF NOT EXISTS FOR (c:Chunk) ON (c.clause_id)"
  log "Neo4j schema is ready"
}

start_stack_if_needed() {
  if [[ "${AUTO_START_STACK}" != "true" ]]; then
    log "AUTO_START_STACK is false; skipping stack startup"
    return
  fi

  if [[ ! -f "${COMPOSE_FILE}" ]]; then
    log "compose file not found: ${COMPOSE_FILE}"
    exit 1
  fi

  local compose_cmd
  compose_cmd="$(compose_binary)"
  log "starting stack from ${COMPOSE_FILE}"
  NEO4J_AUTH="${NEO4J_AUTH}" run_compose up -d
}

start_stack_if_needed
wait_for_http_service "OpenSearch" "${OPENSEARCH_URL}/_cluster/health?wait_for_status=yellow&timeout=1s" -X GET
wait_for_http_service "Qdrant" "${QDRANT_URL}/healthz" -X GET
wait_for_http_service \
  "Neo4j" \
  "${NEO4J_URL}/db/${NEO4J_DB}/tx/commit" \
  -X POST \
  -u "${NEO4J_USER}:${NEO4J_PASSWORD}" \
  -H 'Content-Type: application/json' \
  -d '{"statements":[{"statement":"RETURN 1 AS ok"}]}'

ensure_opensearch_index
ensure_qdrant_collection
ensure_neo4j_schema

log "setup complete"
