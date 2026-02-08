const DEFAULT_API_BASE = "/control-plane";

export function getApiBase() {
  return window.CONTROL_PLANE_API_BASE || DEFAULT_API_BASE;
}

export function getApiKey() {
  return window.CONTROL_PLANE_API_KEY || "";
}

async function fetchJson(path) {
  const base = getApiBase();
  const response = await fetch(`${base}${path}`);
  if (!response.ok) {
    throw new Error(`Control-plane request failed: ${response.status}`);
  }
  return response.json();
}

async function fetchJsonWithAuth(path, options = {}) {
  const base = getApiBase();
  const headers = new Headers(options.headers ?? {});
  const apiKey = getApiKey();
  if (apiKey) {
    headers.set("x-api-key", apiKey);
  }
  const response = await fetch(`${base}${path}`, { ...options, headers });
  if (!response.ok) {
    throw new Error(`Control-plane request failed: ${response.status}`);
  }
  if (response.status === 204) {
    return null;
  }
  return response.json();
}

export async function getRiskHeatmap() {
  return fetchJson("/risk/heatmap");
}

export async function getJobTimeline() {
  return fetchJson("/jobs/live");
}

export async function getPolicyProfiles() {
  return fetchJson("/policies/profiles");
}

export async function getAssets() {
  return fetchJson("/assets");
}

export async function getAuditEvents() {
  return fetchJson("/audit/events");
}

export async function startExtractJob(payload) {
  return fetchJsonWithAuth("/jobs/extract", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload)
  });
}

export async function streamExtractionEvents({ onEvent, onError, signal }) {
  const base = getApiBase();
  const headers = new Headers();
  const apiKey = getApiKey();
  if (apiKey) {
    headers.set("x-api-key", apiKey);
  }

  const response = await fetch(`${base}/events/stream`, { headers, signal });
  if (!response.ok) {
    throw new Error(`Event stream failed: ${response.status}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8");
  let buffer = "";

  while (true) {
    const { value, done } = await reader.read();
    if (done) {
      break;
    }
    buffer += decoder.decode(value, { stream: true });
    let delimiterIndex = buffer.indexOf("\n\n");
    while (delimiterIndex !== -1) {
      const chunk = buffer.slice(0, delimiterIndex);
      buffer = buffer.slice(delimiterIndex + 2);
      const dataLines = chunk
        .split("\n")
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.replace(/^data:\s?/, ""));
      if (dataLines.length) {
        const data = dataLines.join("\n");
        try {
          onEvent?.(JSON.parse(data));
        } catch (error) {
          onError?.(error);
        }
      }
      delimiterIndex = buffer.indexOf("\n\n");
    }
  }
}
