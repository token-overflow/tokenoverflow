# DNS

## Initial Setup

**Step 1:** Get your Cloudflare Zone ID

1. Log in to the [Cloudflare dashboard](https://dash.cloudflare.com)
2. Click on your domain name
3. On the Overview page, look at the right sidebar, copy the Zone ID
4. Replace `REPLACE_WITH_ZONE_ID` in
   `infra/terraform/live/prod/dns/terragrunt.hcl` with that value

**Step 2:** Create a Cloudflare API Token

1. Cloudflare dashboard -> My Profile (top right) -> API Tokens
2. Click Create Token
3. Use the Edit zone DNS template
4. Scope it to your domain name's zone only
5. Click Create Token and copy the token value

**Step 3:** Set Cloudflare SSL mode to Full (Strict)

1. Cloudflare dashboard -> your domain name -> SSL/TLS
2. Set encryption mode to Full (Strict)
3. Optionally: Edge Certificates -> Minimum TLS Version -> TLS 1.3

**Step 4:** Add the token to GitHub Actions

1. GitHub repo -> Settings -> Secrets and variables -> Actions
2. Add new repository secret: `CLOUDFLARE_API_TOKEN` with the token from Step 2
