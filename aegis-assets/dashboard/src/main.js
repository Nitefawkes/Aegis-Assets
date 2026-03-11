import {
  getApiBase,
  getApiKey,
  getAssets,
  getAuditEvents,
  getJobTimeline,
  getPolicyProfiles,
  getRiskHeatmap,
  startExtractJob,
  streamExtractionEvents,
  verifyAuditForJob
} from "./controlPlaneApi.js";
import { createElement } from "./utils/dom.js";
import { RiskHeatmap } from "./components/RiskHeatmap.js";
import { JobTimeline } from "./components/JobTimeline.js";
import { PolicyStudio } from "./components/PolicyStudio.js";
import { AssetGallery } from "./components/AssetGallery.js";
import { AuditTimeline } from "./components/AuditTimeline.js";
import { OperationsConsole } from "./components/OperationsConsole.js";

const main = document.getElementById("main");
const apiBaseLabel = document.getElementById("api-base");

apiBaseLabel.textContent = getApiBase();

const state = {
  risk: { profiles: [], projects: [], publishers: [] },
  jobs: { jobs: [] },
  policies: { profiles: [] },
  assets: { assets: [] },
  audit: {
    events: [],
    filters: { query: "", category: "All", actor: "All", categories: ["All"], actors: ["All"] }
  },
  operations: {
    statusMessages: [],
    events: [],
    compliance: null,
    lastJobId: null,
    jobs: [],
    streamStatus: "Connecting",
    selectedJobId: null,
    lastStreamError: null,
    lastSubmissionError: null,
    streamConnectedAt: null,
    lastAuditVerification: null
  },
  status: null
};

const OPERATIONS_JOBS_KEY = "aegis.operations.jobs";

const fallbackData = {
  risk: {
    profiles: [
      { title: "Core IP", score: 91, status: "High", description: "2 open exceptions" },
      { title: "UGC", score: 76, status: "Medium", description: "3 pending reviews" }
    ],
    projects: [
      { title: "Project Atlas", score: 88, status: "High", description: "ML moderation pass" },
      { title: "Retroverse", score: 62, status: "Watch", description: "Policy drift detected" }
    ],
    publishers: [
      { title: "Lumen", score: 94, status: "High", description: "Compliant" },
      { title: "Nova", score: 71, status: "Medium", description: "Awaiting attestations" }
    ]
  },
  jobs: {
    jobs: [
      {
        name: "Asset ingestion batch 42",
        status: "Running",
        owner: "Pipeline Ops",
        timeline: [
          { title: "Discovery", detail: "Queued 128 assets", timestamp: Date.now() - 1000 * 60 * 45 },
          { title: "Policy scan", detail: "46 assets flagged", timestamp: Date.now() - 1000 * 60 * 25 },
          { title: "Publisher review", detail: "Awaiting approver", timestamp: Date.now() - 1000 * 60 * 5 }
        ]
      }
    ]
  },
  policies: {
    profiles: [
      {
        name: "Global Compliance",
        summary: "Tier-1 policy set for global releases.",
        owner: "Policy Ops",
        nodes: [
          { name: "Identity checks", description: "Verify asset ownership chains.", controls: 6 },
          { name: "Content integrity", description: "Scan for forbidden IP.", controls: 9 },
          { name: "Regional licensing", description: "Route to locale approvals.", controls: 4 }
        ]
      }
    ]
  },
  assets: {
    assets: [
      {
        name: "Starfall Character Pack",
        description: "Approved for EU + NA release.",
        previewLabel: "Character models",
        provenance: "Publisher attested"
      },
      {
        name: "Cityscape Kit",
        description: "AI generated with human review.",
        previewLabel: "Environment assets",
        provenance: "Model card on file"
      }
    ]
  },
  audit: {
    filters: {
      query: "",
      category: "All",
      actor: "All",
      categories: ["All", "Policy", "Asset", "Job"],
      actors: ["All", "Compliance", "Pipeline", "Publisher"]
    },
    events: [
      {
        title: "Policy profile updated",
        detail: "Global Compliance policy now requires attestations.",
        actor: "Compliance",
        category: "Policy",
        timestamp: Date.now() - 1000 * 60 * 90
      },
      {
        title: "Asset approved",
        detail: "Starfall Character Pack cleared for launch.",
        actor: "Publisher",
        category: "Asset",
        timestamp: Date.now() - 1000 * 60 * 30
      }
    ]
  }
};

async function loadData() {
  try {
    const [risk, jobs, policies, assets, audit] = await Promise.all([
      getRiskHeatmap(),
      getJobTimeline(),
      getPolicyProfiles(),
      getAssets(),
      getAuditEvents()
    ]);

    state.risk = risk;
    state.jobs = jobs;
    state.policies = policies;
    state.assets = assets;
    state.audit = {
      events: audit.events ?? [],
      filters: {
        query: "",
        category: "All",
        actor: "All",
        categories: audit.filters?.categories ?? fallbackData.audit.filters.categories,
        actors: audit.filters?.actors ?? fallbackData.audit.filters.actors
      }
    };
    state.status = null;
  } catch (error) {
    state.risk = fallbackData.risk;
    state.jobs = fallbackData.jobs;
    state.policies = fallbackData.policies;
    state.assets = fallbackData.assets;
    state.audit = fallbackData.audit;
    state.status = "Using cached sample data. Control-plane API unavailable.";
  }
}

function updateOperationsStatus(messages) {
  state.operations.statusMessages = messages;
}

function loadSavedJobs() {
  try {
    const saved = window.localStorage.getItem(OPERATIONS_JOBS_KEY);
    if (!saved) {
      return;
    }
    const jobs = JSON.parse(saved);
    if (Array.isArray(jobs)) {
      state.operations.jobs = jobs;
      state.operations.selectedJobId = jobs[0]?.id ?? null;
    }
  } catch (error) {
    console.error("Failed to load saved jobs", error);
  }
}

function saveJobs() {
  try {
    window.localStorage.setItem(OPERATIONS_JOBS_KEY, JSON.stringify(state.operations.jobs));
  } catch (error) {
    console.error("Failed to persist jobs", error);
  }
}

function selectedJob() {
  return state.operations.jobs.find((job) => job.id === state.operations.selectedJobId) ?? null;
}

function selectedJobEvents() {
  const job = selectedJob();
  if (!job) {
    return state.operations.events;
  }
  return state.operations.events.filter((event) => event.jobId === job.id);
}

function updateStreamStatus(status) {
  state.operations.streamStatus = status;
}

function updateStreamError(error) {
  state.operations.lastStreamError = error;
}

function updateSubmissionError(error) {
  state.operations.lastSubmissionError = error;
}

function recordComplianceDecision(decision) {
  state.operations.compliance = {
    status: decision.is_compliant ? "Compliant" : "Blocked",
    riskLevel: decision.risk_level ?? "Unknown",
    warnings: decision.warnings ?? [],
    recommendations: decision.recommendations ?? []
  };
}

function formatEvent(event) {
  const jobId = event.job_id;
  const kind = event.kind;
  if (kind?.JobStateChange) {
    syncJobStatus(jobId, kind.JobStateChange.state, kind.JobStateChange.message ?? "");
    return {
      title: `Job ${kind.JobStateChange.state}`,
      detail: kind.JobStateChange.message ?? "",
      kind: "Job state",
      timestamp: event.occurred_at,
      jobId
    };
  }
  if (kind?.ComplianceDecision) {
    recordComplianceDecision(kind.ComplianceDecision);
    return {
      title: "Compliance decision",
      detail: (kind.ComplianceDecision.warnings ?? []).join(" ") || "No warnings.",
      kind: "Compliance",
      timestamp: event.occurred_at,
      jobId
    };
  }
  if (kind?.AssetIndexingProgress) {
    return {
      title: "Indexing assets",
      detail: `${kind.AssetIndexingProgress.indexed} / ${kind.AssetIndexingProgress.total} indexed`,
      kind: "Indexing",
      timestamp: event.occurred_at,
      jobId
    };
  }
  return {
    title: "Extraction event",
    detail: JSON.stringify(event),
    kind: "Event",
    timestamp: event.occurred_at,
    jobId
  };
}

function syncJobStatus(jobId, status, message) {
  if (!jobId) {
    return;
  }
  const index = state.operations.jobs.findIndex((job) => job.id === jobId);
  if (index === -1) {
    return;
  }
  const current = state.operations.jobs[index];
  state.operations.jobs[index] = {
    ...current,
    status,
    lastMessage: message,
    updatedAt: Date.now()
  };
  saveJobs();
}

async function startOperationsStream() {
  const statusMessages = [];
  statusMessages.push(`API base: ${getApiBase()}`);
  const apiKey = getApiKey();
  statusMessages.push(apiKey ? "API key configured." : "API key missing.");
  updateStreamStatus("Connecting");
  updateOperationsStatus(statusMessages);

  try {
    await streamExtractionEvents({
      onEvent: (event) => {
        const formatted = formatEvent(event);
        state.operations.events = [formatted, ...state.operations.events].slice(0, 25);
        updateStreamStatus("Connected");
        updateStreamError(null);
        state.operations.streamConnectedAt = Date.now();
        if (window.location.hash.replace("#", "") === "operations") {
          renderOperations();
        }
      },
      onError: (error) => {
        console.error(error);
        updateStreamStatus("Error");
        updateStreamError(error?.message ?? "Unknown stream error");
      }
    });
  } catch (error) {
    updateStreamStatus("Disconnected");
    updateStreamError(error?.message ?? "Event stream unavailable.");
    updateOperationsStatus([...statusMessages, "Event stream unavailable."]);
  }
}

function renderPage(title, description, content) {
  main.innerHTML = "";
  const header = createElement("div", {
    className: "page-header",
    children: [
      createElement("div", {
        children: [createElement("h1", { text: title }), createElement("p", { text: description })]
      })
    ]
  });
  main.appendChild(header);

  if (state.status) {
    main.appendChild(createElement("div", { className: "status-banner", text: state.status }));
  }

  main.appendChild(content);
}

const routes = {
  operations: {
    title: "Operations console",
    description: "Submit extraction jobs and monitor live compliance events.",
    render: () =>
      OperationsConsole({
        data: {
          ...state.operations,
          selectedEvents: selectedJobEvents()
        },
        onSubmitJob: handleJobSubmit,
        onRefresh: () => renderOperations(),
        onSelectJob: handleSelectJob,
        onRetryJob: handleRetryJob,
        onRemoveJob: handleRemoveJob,
        onClearJobs: handleClearJobs,
        onReconnect: handleReconnect,
        onVerifyAudit: handleVerifyAudit
      })
  },
  risk: {
    title: "Risk heatmap",
    description: "Live risk posture by profile, project, and publisher.",
    render: () => RiskHeatmap({ data: state.risk })
  },
  jobs: {
    title: "Live job monitor",
    description: "Step-by-step pipeline state for in-flight jobs.",
    render: () => JobTimeline({ data: state.jobs })
  },
  policy: {
    title: "Policy studio",
    description: "Visual editor for compliance profiles and controls.",
    render: () => PolicyStudio({ data: state.policies })
  },
  gallery: {
    title: "Asset gallery",
    description: "Preview assets and review provenance metadata.",
    render: () => AssetGallery({ data: state.assets })
  },
  audit: {
    title: "Audit timeline",
    description: "Filterable audit trail for compliance events.",
    render: () =>
      AuditTimeline({
        data: state.audit,
        onFilterChange: handleAuditFilterChange
      })
  }
};

function setActiveNav(route) {
  document.querySelectorAll(".nav__link").forEach((link) => {
    link.classList.toggle("is-active", link.getAttribute("href") === `#${route}`);
  });
}

function applyAuditFilters() {
  const { query, category, actor } = state.audit.filters;
  const events = state.audit.events.length ? state.audit.events : fallbackData.audit.events;
  return events.filter((event) => {
    const matchesQuery = query
      ? `${event.title} ${event.detail}`.toLowerCase().includes(query.toLowerCase())
      : true;
    const matchesCategory = category && category !== "All" ? event.category === category : true;
    const matchesActor = actor && actor !== "All" ? event.actor === actor : true;
    return matchesQuery && matchesCategory && matchesActor;
  });
}

function handleAuditFilterChange(update) {
  state.audit.filters = { ...state.audit.filters, ...update };
  const filteredEvents = applyAuditFilters();

  const auditContent = AuditTimeline({
    data: { ...state.audit, events: filteredEvents },
    onFilterChange: handleAuditFilterChange
  });
  renderPage(routes.audit.title, routes.audit.description, auditContent);
}

async function navigate() {
  const route = window.location.hash.replace("#", "") || "operations";
  const page = routes[route] ?? routes.risk;
  setActiveNav(route);
  if (route !== "operations") {
    await loadData();
  }
  if (route === "audit") {
    const filteredEvents = applyAuditFilters();
    renderPage(page.title, page.description, AuditTimeline({
      data: { ...state.audit, events: filteredEvents },
      onFilterChange: handleAuditFilterChange
    }));
    return;
  }
  renderPage(page.title, page.description, page.render());
}

window.addEventListener("hashchange", navigate);

function renderOperations() {
  renderPage(routes.operations.title, routes.operations.description, routes.operations.render());
}

async function handleJobSubmit(payload) {
  try {
    const response = await startExtractJob(payload);
    state.operations.lastJobId = response?.job_id ?? null;
    state.operations.jobs = [
      {
        id: state.operations.lastJobId ?? "unknown",
        source: payload.source_path,
        output: payload.output_dir,
        submittedAt: Date.now(),
        status: "Submitted",
        lastMessage: "Job queued"
      },
      ...state.operations.jobs
    ].slice(0, 5);
    state.operations.selectedJobId = state.operations.jobs[0]?.id ?? null;
    updateSubmissionError(null);
    saveJobs();
    updateOperationsStatus([
      `Job submitted: ${state.operations.lastJobId ?? "unknown"}`,
      response?.ownership_verified
        ? "Ownership verification passed."
        : "Ownership verification not required.",
      ...state.operations.statusMessages
    ]);
    renderOperations();
  } catch (error) {
    updateSubmissionError(error?.message ?? "Job submission failed.");
    updateOperationsStatus([`Job submission failed: ${error.message}`]);
    renderOperations();
  }
}

function handleSelectJob(jobId) {
  state.operations.selectedJobId = jobId;
  renderOperations();
}

async function handleRetryJob(job) {
  if (!job) {
    return;
  }
  await handleJobSubmit({ source_path: job.source, output_dir: job.output });
}

function handleRemoveJob(jobId) {
  state.operations.jobs = state.operations.jobs.filter((job) => job.id !== jobId);
  state.operations.selectedJobId = state.operations.jobs[0]?.id ?? null;
  saveJobs();
  renderOperations();
}

function handleClearJobs() {
  state.operations.jobs = [];
  state.operations.selectedJobId = null;
  saveJobs();
  renderOperations();
}


async function handleVerifyAudit(jobId) {
  if (!jobId) {
    return;
  }

  try {
    const result = await verifyAuditForJob(jobId);
    state.operations.lastAuditVerification = {
      jobId,
      verified: Boolean(result?.verified),
      checkedAt: Date.now(),
      error: null
    };
    updateOperationsStatus([`Audit verification succeeded for ${jobId}`]);
  } catch (error) {
    state.operations.lastAuditVerification = {
      jobId,
      verified: false,
      checkedAt: Date.now(),
      error: error?.message ?? "Audit verification failed."
    };
    updateOperationsStatus([`Audit verification failed for ${jobId}`]);
  }

  renderOperations();
}

function handleReconnect() {
  if (state.operations.streamStatus === "Connecting") {
    return;
  }
  startOperationsStream();
  renderOperations();
}

loadSavedJobs();
startOperationsStream();
navigate();
