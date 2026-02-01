# Terraform Guidelines

Follow these rules for Terraform files:

- Use kebab-case for all resource names, identifiers, and string values.
- Avoid "tokenoverflow-" prefix and "-{env}" suffix, each env is a separate AWS
  account; except S3 buckets as they are globally unique.
- Always use latest versions unless there is a specific reason not to.
