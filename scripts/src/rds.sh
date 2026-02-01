#!/usr/bin/env bash

function migration_tunnel() {
  HOST=$(aws rds describe-db-instances \
    --db-instance-identifier main \
    --region us-east-1 \
    --query 'DBInstances[0].Endpoint.Address' \
    --output text \
  )
  TOKENOVERFLOW_RDS_TUNNEL_HOST=$HOST TOKENOVERFLOW_RDS_TUNNEL_PORT=5432 ./scripts/src/rds_tunnel.sh 5432
}

function redo_migrations() {
  diesel migration redo --all \
    --migration-dir apps/api/migrations \
    --database-url "postgres://tokenoverflow:${TOKENOVERFLOW_DATABASE_PASSWORD}@localhost:5432/tokenoverflow"
}
