# PgBouncer

The PgBouncer module deploys a PgBouncer connection pooler on an EC2 instance,
sitting between Lambda and RDS to prevent connection exhaustion.

## Troubleshoot

Connect via SSM Session Manager:

```shell
INSTANCE_ID=$(aws autoscaling describe-auto-scaling-groups \
    --auto-scaling-group-names "pgbouncer" \
    --query 'AutoScalingGroups[0].Instances[0].InstanceId' \
    --output text)
aws ssm start-session --target "$INSTANCE_ID"
```

Inside the session:

```shell
systemctl status pgbouncer
cat /var/log/user-data.log
psql -h 127.0.0.1 -p 6432 -U pgbouncer pgbouncer -c "SHOW POOLS;"
```

## Important: Hardcoded IP

The PgBouncer ENI IP (`10.0.10.200`) is hardcoded in
`live/prod/lambda/terragrunt.hcl` (not via Terragrunt dependency) to avoid a
circular dependency. If the IP changes, update both the PgBouncer and Lambda
Terragrunt configs.
