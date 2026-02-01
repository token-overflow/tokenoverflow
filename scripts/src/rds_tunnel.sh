#!/usr/bin/env bash

set -euo pipefail

AWS_REGION='us-east-1'
ASG_NAME='bastion'
RDS_HOST="${TOKENOVERFLOW_RDS_TUNNEL_HOST:-10.0.10.200}"
RDS_PORT="${TOKENOVERFLOW_RDS_TUNNEL_PORT:-6432}"
LOCAL_PORT="${1:-$RDS_PORT}"
if [ -n "${2:-}" ]; then
  export AWS_PROFILE="$2"
fi

echo '🔍 Retrieving the bastion instance ID.'
INSTANCE_ID=$(aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names "$ASG_NAME" \
  --region "$AWS_REGION" \
  --query "AutoScalingGroups[0].Instances[?LifecycleState==\`InService\`].InstanceId | [0]" \
  --output text)

if [ -z "$INSTANCE_ID" ] || [ "$INSTANCE_ID" = "None" ]; then
  echo "⚠️ Error: No bastion instance found in ASG '${ASG_NAME}'." >&2
  exit 1
fi

echo "🔐 Starting the SSM session."
aws ssm start-session \
  --target "$INSTANCE_ID" \
  --document-name AWS-StartPortForwardingSessionToRemoteHost \
  --parameters "{\"host\":[\"${RDS_HOST}\"],\"portNumber\":[\"${RDS_PORT}\"],\"localPortNumber\":[\"${LOCAL_PORT}\"]}" \
  --region "$AWS_REGION"
