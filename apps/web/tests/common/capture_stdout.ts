import { vi } from "vitest";

/// Captured stdout line. The structured logger emits one
/// newline-delimited JSON record per call; this helper parses each
/// captured line so tests can assert against shaped fields.
export interface CapturedLine {
  level: "info" | "warn" | "error";
  text: string;
}

export interface StdoutCapture {
  lines: CapturedLine[];
  restore: () => void;
}

function deriveLevel(trimmed: string): CapturedLine["level"] {
  try {
    const parsed = JSON.parse(trimmed) as { level?: unknown };
    if (parsed.level === "warn" || parsed.level === "error") {
      return parsed.level;
    }
  } catch {
    // Non-JSON line (vite dev banner etc.); default to "info" so the
    // line is still captured for inspection.
  }
  return "info";
}

/// Spy on `process.stdout.write` and capture each emitted line as a
/// `{ level, text }` record. Used by integration tests that exercise
/// the real route handlers, which transitively import the pino-lambda
/// logger writing to stdout. Unit tests of the logger module itself
/// should use `pino-test`'s `sink()` instead.
export function captureStdout(): StdoutCapture {
  const lines: CapturedLine[] = [];
  // Pino-lambda's destination calls `process.stdout.write(payload)`
  // in sync mode. The payload is a complete newline-delimited JSON
  // line (newlines inside payload values are replaced with carriage
  // returns by pino-lambda) so one write equals one log line.
  const spy = vi.spyOn(process.stdout, "write").mockImplementation(((chunk: unknown) => {
    const text = typeof chunk === "string" ? chunk : (chunk as Buffer).toString("utf8");
    for (const raw of text.split("\n")) {
      const trimmed = raw.trim();
      if (trimmed.length > 0) {
        lines.push({ level: deriveLevel(trimmed), text: trimmed });
      }
    }
    // Swallow the write: forwarding to the real stdout would pollute
    // vitest's test output. The captured `lines` array is the source
    // of truth for assertions.
    return true;
  }) as never);
  return {
    lines,
    restore: () => {
      spy.mockRestore();
    },
  };
}

/// Find the first log line whose `event` matches the supplied `msg`.
/// Pino emits the event identifier as the `msg` field. Returns
/// `undefined` when no line matches.
export function findEvent(
  lines: CapturedLine[],
  event: string,
): Record<string, unknown> | undefined {
  for (const line of lines) {
    let parsed: Record<string, unknown> | undefined;
    try {
      parsed = JSON.parse(line.text) as Record<string, unknown>;
    } catch {
      parsed = undefined;
    }
    if (parsed !== undefined && parsed["msg"] === event) {
      return parsed;
    }
  }
  return undefined;
}

/// Count the number of log lines whose `msg` matches the supplied
/// event identifier.
export function countEvents(lines: CapturedLine[], event: string): number {
  let count = 0;
  for (const line of lines) {
    let parsed: Record<string, unknown> | undefined;
    try {
      parsed = JSON.parse(line.text) as Record<string, unknown>;
    } catch {
      parsed = undefined;
    }
    if (parsed !== undefined && parsed["msg"] === event) {
      count += 1;
    }
  }
  return count;
}
