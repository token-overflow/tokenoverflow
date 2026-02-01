#!/usr/bin/env bash

function redeploy_local() {
  docker compose down -v
  docker compose up -d
  curl http://localhost:8080/health
}
