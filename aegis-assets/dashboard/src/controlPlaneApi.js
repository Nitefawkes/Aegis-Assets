const DEFAULT_API_BASE = "/control-plane";

export function getApiBase() {
  return window.CONTROL_PLANE_API_BASE || DEFAULT_API_BASE;
}

async function fetchJson(path) {
  const base = getApiBase();
  const response = await fetch(`${base}${path}`);
  if (!response.ok) {
    throw new Error(`Control-plane request failed: ${response.status}`);
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
