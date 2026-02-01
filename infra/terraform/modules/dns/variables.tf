variable "cloudflare_zone_id" {
  description = "Cloudflare zone ID for tokenoverflow.io"
  type        = string
}

variable "domains" {
  description = "Map of domains to configure"
  type = map(object({
    domain_name = string
    proxied     = optional(bool, true)
    backend = object({
      type       = string # "api_gateway"
      api_id     = optional(string)
      stage_name = optional(string)
    })
  }))
}
