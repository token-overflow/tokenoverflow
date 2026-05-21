# @tokenoverflow/web

The web BFF (Backend for Frontend).

## Waitlist OAuth flow

The BFF is a confidential WorkOS client (`TokenOverflow Web`,
`client_id = client_01KQZW2FG777B71ZK5WG9EKPTW`). It exchanges the OAuth
`code` for a short-lived JWT via a raw `fetch()` against AuthKit's
per-app `/oauth2/token` endpoint, then forwards the JWT to the Rust API
which writes the waitlist row.

```mermaid
sequenceDiagram
    participant U as User browser
    participant LP as Landing
    participant W as Web BFF
    participant AK as WorkOS AuthKit
    participant API as API

    U->>LP: GET https://tokenoverflow.io/
    LP-->>U: 200 HTML + JS bundle
    U->>W: GET /auth/start?intent=waitlist
    note over W: Generate state.<br/>Sign HS256 JWT cookie __Host-oauth_state.<br/>HttpOnly Secure SameSite=Lax, 10 min TTL.
    W-->>U: 302 -> AuthKit /oauth2/authorize?client_id=...&scope=openid+profile&state=...
    U->>AK: GET /oauth2/authorize?...
    note over AK: AuthKit drives GitHub OAuth.
    AK-->>U: 302 -> /auth/callback?code=...&state=...
    U->>W: GET /auth/callback?code=...&state=...
    note over W: Verify and clear state cookie.
    W->>AK: POST /oauth2/token (form-encoded)<br/>grant_type, code, client_id, client_secret, redirect_uri
    AK-->>W: 200 { access_token, token_type, expires_in }
    W->>API: POST /v1/waitlist<br/>Authorization: Bearer access_token
    API-->>W: 201 { github_id, github_username, already_on_waitlist }
    W-->>U: 302 -> https://tokenoverflow.io/?waitlist=success
    U->>LP: GET /?waitlist=success
    LP-->>U: 200 HTML + JS bundle
    note over U: JS reads query param, shows green popup
```
