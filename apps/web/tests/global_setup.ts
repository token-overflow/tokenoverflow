import net from "node:net";

/// Playwright globalSetup hook. Verifies the local stack is reachable before
/// any spec runs so we fail with a clear message instead of letting specs
/// time out against a closed port. The cross-stack spec drives both web
/// (3000) and landing (4321), so we probe both. Run `redeploy_local` first
/// (or the explicit docker compose command shown below) to bring the stack
/// up.
const REQUIRED_PORTS = [3000, 4321];
const HOST = "localhost";
const CONNECT_TIMEOUT_MS = 1000;

async function isPortOpen(port: number, host: string): Promise<boolean> {
  // oxlint-disable-next-line promise/avoid-new
  return new Promise((resolve) => {
    const socket = net.createConnection({ port, host });
    const finish = (open: boolean) => {
      socket.destroy();
      resolve(open);
    };
    socket.setTimeout(CONNECT_TIMEOUT_MS);
    socket.once("connect", () => finish(true));
    socket.once("timeout", () => finish(false));
    socket.once("error", () => finish(false));
  });
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
