import { once } from "node:events";
import net from "node:net";

/// Fail fast with a clear message if the local stack isn't up before any spec runs.
const REQUIRED_PORTS = [3000, 4321];
const HOST = "localhost";
const CONNECT_TIMEOUT_MS = 1000;

async function isPortOpen(port: number, host: string): Promise<boolean> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), CONNECT_TIMEOUT_MS);
  const socket = net.createConnection({ port, host });
  try {
    await once(socket, "connect", { signal: controller.signal });
    return true;
  } catch {
    return false;
  } finally {
    clearTimeout(timeout);
    socket.destroy();
  }
}

export default async function globalSetup(): Promise<void> {
  for (const port of REQUIRED_PORTS) {
    if (!(await isPortOpen(port, HOST))) {
      throw new Error(
        "The local stack is not up. Run `redeploy_local` or `docker compose --profile web up -d --wait`.",
      );
    }
  }
}
