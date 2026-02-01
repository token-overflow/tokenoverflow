# NAT (fck-nat)

The fck-nat module (`modules/nat/`) provides cost-effective outbound internet
access for private subnets using the [fck-nat](https://fck-nat.dev/) open-source
NAT instance AMI.

**Important:** The VPC module's `enable_nat_gateway` must remain `false`.
If migrating to a managed NAT Gateway, destroy the fck-nat unit first:

```shell
cd infra/terraform/live/prod/nat
terragrunt destroy
```

Then enable `enable_nat_gateway = true` in the VPC module.
