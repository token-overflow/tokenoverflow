# RDS

## Rotate Password

```shell
export TF_VAR_password_wo='<new-password>'
export TF_VAR_password_wo_version=2
cd infra/terraform/live/prod/rds
terragrunt apply
```
