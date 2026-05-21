import { createPrivateKey, createSign } from "node:crypto";
import { readFile } from "node:fs/promises";

import { config } from "@tokenoverflow/config";

/// Path inside the BFF docker container where the API's test private key
/// is mounted read-only. See `docker-compose.yml` web service definition.
const LOCAL_PRIVATE_KEY_PATH = "/app/tests/assets/auth/test_private_key.pem";

/// `kid` matching the test JWKS at
/// `apps/api/tests/assets/auth/test_jwks.json`. Hardcoded to mirror the
/// Rust API's existing test helpers in
/// `apps/api/tests/common/test_jwt.rs`.
const TEST_KID = "test-key-1";

/// Standard JWT claims for the local stub. `iss` and `aud` match
/// `apps/api/config/local.toml`.
const LOCAL_ISSUER = "tokenoverflow-test";
const LOCAL_AUDIENCE = "http://localhost:8080";

/// JWT TTL matches the prod AuthKit access_token duration.
const LOCAL_TTL_SECONDS = 300;

/// Stub email claim. The API rejects tokens missing `email`, so the
/// local-mode JWT must carry one to match the prod scope shape.
const LOCAL_EMAIL = "test-voter@example.test";

function base64UrlEncodeJson(value: object): string {
  const json = JSON.stringify(value);
  return Buffer.from(json, "utf8").toString("base64url");
}

/// Read the mounted private key. Cached per process so subsequent
/// signings skip the filesystem.
let cachedPrivateKeyPem: Promise<string> | undefined;
async function loadPrivateKey(override_path?: string): Promise<string> {
  const path = override_path ?? LOCAL_PRIVATE_KEY_PATH;
  if (override_path) {
    // Bypass the cache for explicit path overrides (used in unit tests).
    return await readFile(path, "utf8");
  }
  cachedPrivateKeyPem ??= readFile(path, "utf8");
  return await cachedPrivateKeyPem;
}

export interface SignLocalJwtOptions {
  /// Override the `sub` claim. Defaults to `config.web.workos_id` (the
  /// seeded `test-voter` `workos_id` in local mode).
  sub?: string;
  /// Override the `email` claim. Defaults to `LOCAL_EMAIL`.
  email?: string;
  /// Override the private key path. Used by unit tests to load the
  /// repo-relative path; production uses the docker mount path.
  privateKeyPath?: string;
  /// Override `now()` for deterministic tests.
  now?: number;
}

/// Synthesize a JWT signed with the API's local test private key. The
/// produced token validates against the static JWKS in
/// `apps/api/tests/assets/auth/test_jwks.json` so the API's
/// `jwt_auth_layer` accepts it transparently.
///
/// This module **must only** be imported behind a
/// `config.web.auth_mode === "bypass"` branch. The dynamic import in
/// the OAuth strategy module prevents the production bundle from pulling
/// the test key path into its dependency graph.
export async function signLocalAuthJwt(opts: SignLocalJwtOptions = {}): Promise<string> {
  const pem = await loadPrivateKey(opts.privateKeyPath);
  const private_key = createPrivateKey({ key: pem });

  const now = opts.now ?? Math.floor(Date.now() / 1_000);
  const sub = opts.sub ?? config.web.workos_id;
  const email = opts.email ?? LOCAL_EMAIL;

  const header = {
    alg: "RS256",
    typ: "JWT",
    kid: TEST_KID,
  } as const;

  const payload = {
    sub,
    email,
    iss: LOCAL_ISSUER,
    aud: LOCAL_AUDIENCE,
    iat: now,
    exp: now + LOCAL_TTL_SECONDS,
  };

  const signing_input = `${base64UrlEncodeJson(header)}.${base64UrlEncodeJson(payload)}`;
  const signer = createSign("RSA-SHA256");
  signer.update(signing_input);
  const signature = signer.sign(private_key).toString("base64url");
  return `${signing_input}.${signature}`;
}
