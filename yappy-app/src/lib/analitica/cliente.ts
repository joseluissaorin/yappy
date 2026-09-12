// El cliente mínimo del worker de analítica (copiado del SDK de la casa,
// ~/Developer/analytics/sdk, sin dependencias): guarda una cola, la
// persiste, y la vacía por lotes con reintento. Aquí el almacén es
// localStorage del webview.

export interface KVStorage {
  getItem(key: string): Promise<string | null> | string | null;
  setItem(key: string, value: string): Promise<void> | void;
}

export interface AnalyticsOptions {
  endpoint: string;
  appToken: string;
  storage?: KVStorage;
  appVersion?: string;
  platform?: string;
  flushIntervalMs?: number;
  maxQueue?: number;
  sessionGapMs?: number;
  debug?: boolean;
}

interface Ev {
  event: string;
  distinct_id: string;
  session_id: string;
  ts: number;
  props?: Record<string, unknown>;
  platform?: string;
  app_version?: string;
}

const KEY_ID = "@analytics/did";
const KEY_QUEUE = "@analytics/queue";

function uuid(): string {
  const g = globalThis as unknown as { crypto?: { randomUUID?: () => string } };
  if (g.crypto?.randomUUID) return g.crypto.randomUUID();
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === "x" ? r : (r & 0x3) | 0x8).toString(16);
  });
}

export function createAnalytics(opts: AnalyticsOptions) {
  const {
    endpoint,
    appToken,
    storage,
    appVersion,
    platform,
    flushIntervalMs = 15000,
    maxQueue = 20,
    sessionGapMs = 30 * 60 * 1000,
    debug = false,
  } = opts;

  const log = (...a: unknown[]) => {
    if (debug) console.log("[analitica]", ...a);
  };
  const url = endpoint.replace(/\/+$/, "") + "/e";

  let distinctId = "";
  let sessionId = uuid();
  let lastActivity = Date.now();
  let queue: Ev[] = [];
  let flushing = false;

  async function load() {
    try {
      let did = (await storage?.getItem(KEY_ID)) || "";
      if (!did) {
        did = uuid();
        await storage?.setItem(KEY_ID, did);
      }
      distinctId = did;
      const raw = await storage?.getItem(KEY_QUEUE);
      if (raw) queue = JSON.parse(raw);
    } catch (e) {
      log("load error", e);
      distinctId = distinctId || uuid();
    }
  }
  const ready: Promise<void> = load();

  async function persist() {
    try {
      await storage?.setItem(KEY_QUEUE, JSON.stringify(queue.slice(-500)));
    } catch {}
  }

  function touchSession() {
    const now = Date.now();
    if (now - lastActivity > sessionGapMs) sessionId = uuid();
    lastActivity = now;
  }

  async function enqueue(event: string, props?: Record<string, unknown>) {
    await ready;
    touchSession();
    queue.push({
      event,
      distinct_id: distinctId,
      session_id: sessionId,
      ts: Date.now(),
      props,
      platform,
      app_version: appVersion,
    });
    log("queued", event, props ?? "");
    await persist();
    if (queue.length >= maxQueue) void flush();
  }

  async function flush(): Promise<void> {
    await ready;
    if (flushing || queue.length === 0) return;
    flushing = true;
    const batch = queue.slice(0, 100);
    try {
      const res = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${appToken}` },
        body: JSON.stringify({ events: batch }),
      });
      if (res.ok) {
        queue = queue.slice(batch.length);
        await persist();
        log("flushed", batch.length);
      } else {
        log("flush http", res.status);
      }
    } catch (e) {
      log("flush error", e);
    } finally {
      flushing = false;
    }
  }

  const timer = setInterval(() => void flush(), flushIntervalMs);

  return {
    track: (event: string, props?: Record<string, unknown>) => void enqueue(event, props),
    screen: (name: string, props?: Record<string, unknown>) => void enqueue("$screen", { name, ...props }),
    identify: (userId: string) => void enqueue("$identify", { user_id: userId }),
    flush,
    getDistinctId: async () => {
      await ready;
      return distinctId;
    },
    stop: () => clearInterval(timer),
  };
}

export type Analytics = ReturnType<typeof createAnalytics>;
