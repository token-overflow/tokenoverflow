#!/usr/bin/env bash

function redeploy_local() {
  docker compose down -v
  # Profiles gate every service, so each leg must be enabled explicitly.
  docker compose --profile api --profile landing --profile web up -d --build --wait
  curl http://localhost:8080/health
}
