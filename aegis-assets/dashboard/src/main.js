import {
  getApiBase,
  getAssets,
  getAuditEvents,
  getJobTimeline,
  getPolicyProfiles,
  getRiskHeatmap
} from "./controlPlaneApi.js";
import { createElement } from "./utils/dom.js";
import { RiskHeatmap } from "./components/RiskHeatmap.js";
import { JobTimeline } from "./components/JobTimeline.js";
import { PolicyStudio } from "./components/PolicyStudio.js";
import { AssetGallery } from "./components/AssetGallery.js";
import { AuditTimeline } from "./components/AuditTimeline.js";

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
  status: null
};

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
  const route = window.location.hash.replace("#", "") || "risk";
  const page = routes[route] ?? routes.risk;
  setActiveNav(route);
  await loadData();
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

navigate();
