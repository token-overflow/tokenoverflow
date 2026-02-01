#!/usr/bin/env bash

set -euo pipefail

# TODO: Change to non-admin profile once other profiles are created
AWS_PROFILE='tokenoverflow-prod-admin'
AWS_REGION='us-east-1'
ASG_NAME='bastion'
RDS_HOST='10.0.10.200'
RDS_PORT='6432'
LOCAL_PORT="${1:-6432}"

echo '🔍 Retrieving the bastion instance ID.'
INSTANCE_ID=$(aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names "$ASG_NAME" \
  --profile "$AWS_PROFILE" \
  --region "$AWS_REGION" \
  --query "AutoScalingGroups[0].Instances[?LifecycleState==\`InService\`].InstanceId | [0]" \
  --output text)

if [ -z "$INSTANCE_ID" ] || [ "$INSTANCE_ID" = "None" ]; then
  echo "⚠️ Error: No bastion instance found in ASG '${ASG_NAME}'." >&2
  exit 1
fi

echo "🔐 Starting SSH tunnel: localhost:${LOCAL_PORT} -> ${RDS_HOST}:${RDS_PORT}"
exec ssh -N -L "${LOCAL_PORT}:${RDS_HOST}:${RDS_PORT}" \
  "ec2-user@${INSTANCE_ID}" \
  -i ~/.ssh/tokenoverflow_bastion \
  -o StrictHostKeyChecking=no \
  -o UserKnownHostsFile=/dev/null \
  -o ProxyCommand="aws ssm start-session --target %h --document-name AWS-StartSSHSession --parameters portNumber=%p --profile ${AWS_PROFILE} --region ${AWS_REGION}"
