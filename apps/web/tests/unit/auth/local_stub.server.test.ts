import { createPublicKey, createVerify } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

beforeEach(() => {
  vi.stubEnv("TOKENOVERFLOW_ENV", "local");
  // Stub the workos_id BEFORE the dynamic import so the shared config
  // picks the override up at module load. The shared config caches; the
  // `vi.resetModules()` call below evicts the cached instance so the
  // next `await import(...)` re-reads `process.env`.
  vi.stubEnv("TOKENOVERFLOW_WEB_WORKOS_ID", "00000000-0000-0000-0000-000000000002");
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const TEST_ASSETS_DIR = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../api/tests/assets/auth",
);
const TEST_PRIVATE_KEY_PATH = resolve(TEST_ASSETS_DIR, "test_private_key.pem");
const TEST_JWKS_PATH = resolve(TEST_ASSETS_DIR, "test_jwks.json");

interface JwksKey {
  kid: string;
  n: string;
  e: string;
  kty: string;
  alg: string;
  use: string;
}

interface Jwks {
  keys: JwksKey[];
}

function base64UrlDecodeJson<T>(value: string): T {
  return JSON.parse(Buffer.from(value, "base64url").toString("utf8")) as T;
}

async function loadJwksKey(): Promise<JwksKey> {
  const json = await readFile(TEST_JWKS_PATH, "utf8");
  const jwks = JSON.parse(json) as Jwks;
  const key = jwks.keys.find((k) => k.kid === "test-key-1");
  if (!key) {
    throw new Error("test-key-1 not found in test JWKS");
  }
  return key;
}

describe("signLocalAuthJwt", () => {
  it("signs a JWT whose payload matches the documented claims", async () => {
    const { signLocalAuthJwt } = await import("../../../src/utils/auth/local_stub.server");
    const fixed_now = 1_700_000_000;
    const jwt = await signLocalAuthJwt({
      privateKeyPath: TEST_PRIVATE_KEY_PATH,
      now: fixed_now,
    });
    const [header_b64, payload_b64] = jwt.split(".");
    const header = base64UrlDecodeJson<{ alg: string; kid: string }>(header_b64!);
    const payload = base64UrlDecodeJson<{
      sub: string;
      email: string;
      iss: string;
      aud: string;
      iat: number;
      exp: number;
    }>(payload_b64!);

    expect(header.alg).toBe("RS256");
    expect(header.kid).toBe("test-key-1");
    expect(payload.sub).toBe("00000000-0000-0000-0000-000000000002");
    expect(payload.email).toBe("test-voter@example.test");
    expect(payload.iss).toBe("tokenoverflow-test");
    expect(payload.aud).toBe("http://localhost:8080");
    expect(payload.iat).toBe(fixed_now);
    expect(payload.exp).toBe(fixed_now + 300);
  });

  it("verifies against the static JWKS public key", async () => {
    const { signLocalAuthJwt } = await import("../../../src/utils/auth/local_stub.server");
    const jwt = await signLocalAuthJwt({
      privateKeyPath: TEST_PRIVATE_KEY_PATH,
    });
    const [header_b64, payload_b64, signature_b64] = jwt.split(".");

    const jwks_key = await loadJwksKey();
    const public_key = createPublicKey({
      key: {
        kty: jwks_key.kty,
        n: jwks_key.n,
        e: jwks_key.e,
      },
      format: "jwk",
    });

    const verifier = createVerify("RSA-SHA256");
    verifier.update(`${header_b64}.${payload_b64}`);
    const signature = Buffer.from(signature_b64!, "base64url");
    const ok = verifier.verify(public_key, signature);
    expect(ok).toBe(true);
  });

  it("respects the `sub` override", async () => {
    const { signLocalAuthJwt } = await import("../../../src/utils/auth/local_stub.server");
    const jwt = await signLocalAuthJwt({
      privateKeyPath: TEST_PRIVATE_KEY_PATH,
      sub: "00000000-0000-0000-0000-000000000123",
    });
    const [_header, payload_b64] = jwt.split(".");
    const payload = base64UrlDecodeJson<{ sub: string }>(payload_b64!);
    expect(payload.sub).toBe("00000000-0000-0000-0000-000000000123");
  });

  it("respects the `email` override", async () => {
    const { signLocalAuthJwt } = await import("../../../src/utils/auth/local_stub.server");
    const jwt = await signLocalAuthJwt({
      privateKeyPath: TEST_PRIVATE_KEY_PATH,
      email: "override@example.test",
    });
    const [_header, payload_b64] = jwt.split(".");
    const payload = base64UrlDecodeJson<{ email: string }>(payload_b64!);
    expect(payload.email).toBe("override@example.test");
  });
});
