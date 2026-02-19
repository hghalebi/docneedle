bootstrap:
    bash scripts/bootstrap-stack.sh

bootstrap-watch:
    @set -euo pipefail; \
    compose_cmd() { \
      if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then \
        echo docker compose; \
      elif command -v docker-compose >/dev/null 2>&1; then \
        echo docker-compose; \
      else \
        echo "docker compose is required" >&2; \
        exit 1; \
      fi; \
    }; \
    export BOOTSTRAP_LOG=/tmp/pdf-search-bootstrap.log; \
    echo "Logging bootstrap output to ${BOOTSTRAP_LOG}" ; \
    bash scripts/bootstrap-stack.sh 2>&1 | tee "${BOOTSTRAP_LOG}" & \
    BOOTSTRAP_PID=$!; \
    $(compose_cmd) -f deploy/docker-compose.yml logs --no-color -f opensearch qdrant neo4j 2>&1 | tee -a "${BOOTSTRAP_LOG}" & \
    LOG_PID=$!; \
    wait $BOOTSTRAP_PID; \
    BOOTSTRAP_EXIT=$?; \
    kill $LOG_PID 2>/dev/null || true; \
    wait $LOG_PID 2>/dev/null || true; \
    exit $BOOTSTRAP_EXIT
