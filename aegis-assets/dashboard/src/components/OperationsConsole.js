import { createElement, formatTimestamp } from "../utils/dom.js";

export function OperationsConsole({
  data,
  onSubmitJob,
  onRefresh,
  onSelectJob,
  onRetryJob,
  onRemoveJob,
  onClearJobs,
  onReconnect,
  onVerifyAudit
}) {
  const selectedJob = data.jobs.find((job) => job.id === data.selectedJobId) ?? null;
  const statusItems = data.statusMessages.map((message) =>
    createElement("li", { text: message })
  );

  const jobSummary = data.jobs.reduce(
    (acc, job) => {
      const key = (job.status || "Unknown").toLowerCase();
      acc.total += 1;
      if (key.includes("completed")) acc.completed += 1;
      else if (key.includes("failed")) acc.failed += 1;
      else if (key.includes("running")) acc.running += 1;
      else acc.pending += 1;
      return acc;
    },
    { total: 0, completed: 0, failed: 0, running: 0, pending: 0 }
  );

  const allStatuses = ["all", ...new Set(data.jobs.map((job) => String(job.status || "unknown").toLowerCase()))];

  const jobItems = data.jobs.length
    ? data.jobs.map((job) =>
        createElement("li", {
          className: `job-card${job.id === data.selectedJobId ? " is-selected" : ""}`,
          children: [
            createElement("div", {
              className: "job-card__meta",
              children: [
                createElement("span", { className: "tag", text: job.status }),
                createElement("span", { text: formatTimestamp(job.submittedAt) })
              ]
            }),
            createElement("h4", { text: job.id }),
            createElement("p", { text: `Source: ${job.source}` }),
            createElement("p", { text: `Output: ${job.output}` })
          ],
          attributes: { role: "button", tabindex: "0" }
        })
      )
    : [createElement("li", { className: "empty-state", text: "No jobs submitted yet." })];

  jobItems.forEach((item, index) => {
    if (!data.jobs[index]) {
      return;
    }
    const jobId = data.jobs[index].id;
    item.addEventListener("click", () => onSelectJob?.(jobId));
    item.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        onSelectJob?.(jobId);
      }
    });
  });

  const applyFilters = ({ items, query, matcher, emptyLabel }) => {
    let visible = 0;
    items.forEach((item, index) => {
      const record = matcher(index);
      if (!record) {
        return;
      }
      const matches = query(record);
      item.classList.toggle("is-hidden", !matches);
      if (matches) visible += 1;
    });
    return visible || emptyLabel;
  };

  const eventItems = data.selectedEvents.length
    ? data.selectedEvents.map((event) =>
        createElement("li", {
          className: "event-card",
          children: [
            createElement("div", {
              className: "event-card__meta",
              children: [
                createElement("span", { className: "tag", text: event.kind }),
                createElement("span", { text: formatTimestamp(event.timestamp) })
              ]
            }),
            createElement("h4", { text: event.title }),
            createElement("p", { text: event.detail })
          ]
        })
      )
    : [createElement("li", { className: "empty-state", text: "No events for this job yet." })];

  const compliance = data.compliance;
  const complianceCard = createElement("div", {
    className: "card",
    children: [
      createElement("h3", { text: "Latest compliance decision" }),
      createElement("p", {
        text: compliance
          ? `${compliance.riskLevel} • ${compliance.status}`
          : "No compliance decisions received yet."
      }),
      compliance
        ? createElement("ul", {
            className: "list",
            children: compliance.warnings.map((warning) =>
              createElement("li", { text: warning })
            )
          })
        : null,
      compliance?.recommendations?.length
        ? createElement("div", {
            className: "recommendations",
            children: [
              createElement("h4", { text: "Recommendations" }),
              createElement("ul", {
                className: "list",
                children: compliance.recommendations.map((rec) =>
                  createElement("li", { text: rec })
                )
              })
            ]
          })
        : null
    ]
  });

  const jobForm = createElement("form", {
    className: "form",
    children: [
      createElement("div", {
        className: "form__field",
        children: [
          createElement("label", { text: "Source path" }),
          createElement("input", {
            attributes: {
              name: "source_path",
              type: "text",
              placeholder: "/path/to/archive.unity3d",
              required: "true"
            }
          })
        ]
      }),
      createElement("div", {
        className: "form__field",
        children: [
          createElement("label", { text: "Output directory" }),
          createElement("input", {
            attributes: {
              name: "output_dir",
              type: "text",
              placeholder: "/path/to/output",
              required: "true"
            }
          })
        ]
      }),
      createElement("div", {
        className: "form__field",
        children: [
          createElement("label", { text: "Ownership platform (optional)" }),
          createElement("input", {
            attributes: {
              name: "ownership_platform",
              type: "text",
              placeholder: "steam or epic"
            }
          })
        ]
      }),
      createElement("div", {
        className: "form__field",
        children: [
          createElement("label", { text: "Ownership app id (optional)" }),
          createElement("input", {
            attributes: {
              name: "ownership_app_id",
              type: "text",
              placeholder: "570"
            }
          })
        ]
      }),
      createElement("div", {
        className: "form__field",
        children: [
          createElement("label", { text: "Ownership account id (optional)" }),
          createElement("input", {
            attributes: {
              name: "ownership_account_id",
              type: "text",
              placeholder: "acct-1"
            }
          })
        ]
      }),
      createElement("div", {
        className: "form__actions",
        children: [
          createElement("button", {
            className: "button",
            text: "Start extraction",
            attributes: { type: "submit" }
          }),
          createElement("button", {
            className: "button button--ghost",
            text: "Refresh",
            attributes: { type: "button" }
          })
        ]
      })
    ]
  });

  const refreshButton = jobForm.querySelector("button.button--ghost");
  refreshButton?.addEventListener("click", () => onRefresh?.());

  jobForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    const formData = new FormData(jobForm);
    const ownershipPlatform = String(formData.get("ownership_platform") || "").trim();
    const ownershipAppId = String(formData.get("ownership_app_id") || "").trim();
    const ownershipAccountId = String(formData.get("ownership_account_id") || "").trim();
    const payload = {
      source_path: formData.get("source_path"),
      output_dir: formData.get("output_dir")
    };

    if (ownershipPlatform && ownershipAppId && ownershipAccountId) {
      payload.ownership = {
        platform: ownershipPlatform,
        app_id: ownershipAppId,
        account_id: ownershipAccountId
      };
    }
    await onSubmitJob?.(payload);
    jobForm.reset();
  });

  const jobDetails = selectedJob
    ? createElement("div", {
        className: "job-details",
        children: [
          createElement("p", { text: `ID: ${selectedJob.id}` }),
          createElement("p", { text: `Status: ${selectedJob.status}` }),
          createElement("p", { text: `Last message: ${selectedJob.lastMessage ?? "N/A"}` }),
          createElement("p", {
            text: `Updated: ${formatTimestamp(selectedJob.updatedAt ?? selectedJob.submittedAt)}`
          }),
          createElement("button", {
            className: "button button--ghost",
            text: "Retry job",
            attributes: { type: "button" }
          }),
          createElement("button", {
            className: "button button--ghost",
            text: "Verify audit",
            attributes: { type: "button" }
          })
        ]
      })
    : createElement("p", {
        className: "empty-state",
        text: "Select a job to inspect details."
      });

  if (selectedJob) {
    const [retryButton, verifyButton] = jobDetails.querySelectorAll(".button--ghost");
    retryButton?.addEventListener("click", () => onRetryJob?.(selectedJob));
    verifyButton?.addEventListener("click", () => onVerifyAudit?.(selectedJob.id));
    const removeButton = createElement("button", {
      className: "button button--danger",
      text: "Remove from history",
      attributes: { type: "button" }
    });
    removeButton.addEventListener("click", () => onRemoveJob?.(selectedJob.id));
    jobDetails.appendChild(removeButton);
  }

  const clearHistoryButton = createElement("button", {
    className: "button button--ghost",
    text: "Clear history",
    attributes: { type: "button" }
  });
  clearHistoryButton.addEventListener("click", () => onClearJobs?.());

  const reconnectButton = createElement("button", {
    className: "button button--ghost",
    text: "Reconnect stream",
    attributes: { type: "button" }
  });
  reconnectButton.addEventListener("click", () => onReconnect?.());

  const exportButton = createElement("button", {
    className: "button button--ghost",
    text: "Export diagnostics",
    attributes: { type: "button" }
  });
  exportButton.addEventListener("click", () => {
    const payload = {
      generatedAt: new Date().toISOString(),
      streamStatus: data.streamStatus,
      lastStreamError: data.lastStreamError,
      lastSubmissionError: data.lastSubmissionError,
      selectedJob,
      selectedEvents: data.selectedEvents,
      lastAuditVerification: data.lastAuditVerification
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `aegis-diagnostics-${Date.now()}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  });

  const diagnosticsCard = createElement("div", {
    className: "card",
    children: [
      createElement("div", {
        className: "status-row",
        children: [
          createElement("h3", { text: "Diagnostics" }),
          createElement("div", { className: "toolbar", children: [reconnectButton, exportButton] })
        ]
      }),
      createElement("ul", {
        className: "list",
        children: [
          createElement("li", {
            text: `Stream connected: ${data.streamConnectedAt ? formatTimestamp(data.streamConnectedAt) : "Not yet"}`
          }),
          createElement("li", {
            text: `Last stream error: ${data.lastStreamError ?? "None"}`
          }),
          createElement("li", {
            text: `Last submission error: ${data.lastSubmissionError ?? "None"}`
          }),
          createElement("li", {
            text: data.lastAuditVerification
              ? `Last audit check: ${data.lastAuditVerification.jobId} • ${data.lastAuditVerification.verified ? "Verified" : data.lastAuditVerification.error ?? "Failed"}`
              : "Last audit check: None"
          })
        ]
      })
    ]
  });

  const operationsView = createElement("div", {
    className: "operations",
    children: [
      createElement("div", {
        className: "card",
        children: [
          createElement("h3", { text: "Submit extraction" }),
          createElement("p", {
            text: "Create a new extraction job and monitor real-time events."
          }),
          createElement("p", {
            className: "muted",
            text: "Set window.CONTROL_PLANE_API_KEY for authenticated requests."
          }),
          jobForm
        ]
      }),
      createElement("div", {
        className: "card",
        children: [
          createElement("h3", { text: "Control-plane status" }),
          createElement("div", {
            className: "status-row",
            children: [
              createElement("span", { className: "tag", text: data.streamStatus }),
              createElement("span", { text: "Event stream" })
            ]
          }),
          createElement("ul", { className: "list", children: statusItems })
        ]
      }),
      diagnosticsCard,
      createElement("div", {
        className: "card",
        children: [
          createElement("h3", { text: "Job health" }),
          createElement("div", {
            className: "health-grid",
            children: [
              createElement("div", { className: "health-pill", children: [createElement("strong", { text: String(jobSummary.total) }), createElement("span", { text: "Total" })] }),
              createElement("div", { className: "health-pill", children: [createElement("strong", { text: String(jobSummary.running) }), createElement("span", { text: "Running" })] }),
              createElement("div", { className: "health-pill", children: [createElement("strong", { text: String(jobSummary.pending) }), createElement("span", { text: "Pending" })] }),
              createElement("div", { className: "health-pill", children: [createElement("strong", { text: String(jobSummary.completed) }), createElement("span", { text: "Completed" })] }),
              createElement("div", { className: "health-pill", children: [createElement("strong", { text: String(jobSummary.failed) }), createElement("span", { text: "Failed" })] })
            ]
          })
        ]
      }),
      createElement("div", {
        className: "card",
        children: [
          createElement("div", {
            className: "status-row",
            children: [createElement("h3", { text: "Recent jobs" }), clearHistoryButton]
          }),
          createElement("div", {
            className: "filters",
            attributes: { "data-panel": "job-filters" },
            children: [
              createElement("input", {
                className: "input",
                attributes: { type: "search", placeholder: "Search by id/source/output", "aria-label": "Search jobs" }
              }),
              createElement("select", {
                className: "input",
                attributes: { "aria-label": "Filter jobs by status" },
                children: allStatuses.map((status) =>
                  createElement("option", {
                    text: status === "all" ? "All statuses" : status,
                    attributes: { value: status }
                  })
                )
              }),
              createElement("button", {
                className: "button button--ghost button--small",
                text: "Reset",
                attributes: { type: "button", "aria-label": "Reset job filters" }
              }),
              createElement("span", { className: "muted", text: `Showing ${data.jobs.length} jobs` })
            ]
          }),
          createElement("ul", { className: "event-list", children: jobItems })
        ]
      }),
      createElement("div", {
        className: "card",
        children: [
          createElement("h3", { text: "Job details" }),
          jobDetails
        ]
      }),
      complianceCard,
      createElement("div", {
        className: "card",
        children: [
          createElement("h3", {
            text: selectedJob ? `Live event stream (${selectedJob.id})` : "Live event stream"
          }),
          createElement("div", {
            className: "filters",
            attributes: { "data-panel": "event-filters" },
            children: [
              createElement("input", {
                className: "input",
                attributes: { type: "search", placeholder: "Filter selected events", "aria-label": "Filter selected events" }
              }),
              createElement("button", {
                className: "button button--ghost button--small",
                text: "Reset",
                attributes: { type: "button", "aria-label": "Reset event filters" }
              }),
              createElement("span", { className: "muted", text: `${data.selectedEvents.length} events` })
            ]
          }),
          createElement("ul", { className: "event-list", children: eventItems })
        ]
      })
    ]
  });

  const jobFiltersPanel = operationsView.querySelector(`[data-panel="job-filters"]`);
  const eventFiltersPanel = operationsView.querySelector(`[data-panel="event-filters"]`);

  const jobSearchInput = jobFiltersPanel?.querySelector("input");
  const jobStatusSelect = jobFiltersPanel?.querySelector("select");
  const jobResetButton = jobFiltersPanel?.querySelector("button");
  const jobCountLabel = jobFiltersPanel?.querySelector(".muted");
  const eventSearchInput = eventFiltersPanel?.querySelector("input");
  const eventResetButton = eventFiltersPanel?.querySelector("button");
  const eventCountLabel = eventFiltersPanel?.querySelector(".muted");

  const refreshJobFilters = () => {
    const searchValue = (jobSearchInput?.value || "").trim().toLowerCase();
    const selectedStatus = (jobStatusSelect?.value || "all").toLowerCase();
    const visible = applyFilters({
      items: jobItems,
      matcher: (index) => data.jobs[index],
      query: (job) => {
        const status = String(job.status || "").toLowerCase();
        const matchesStatus = selectedStatus === "all" || status === selectedStatus;
        const target = `${job.id} ${job.source} ${job.output}`.toLowerCase();
        const matchesSearch = !searchValue || target.includes(searchValue);
        return matchesStatus && matchesSearch;
      },
      emptyLabel: "No jobs"
    });
    if (jobCountLabel) {
      jobCountLabel.textContent = typeof visible === "number" ? `Showing ${visible} jobs` : visible;
    }
  };

  const refreshEventFilters = () => {
    const searchValue = (eventSearchInput?.value || "").trim().toLowerCase();
    const visible = applyFilters({
      items: eventItems,
      matcher: (index) => data.selectedEvents[index],
      query: (event) => !searchValue || `${event.kind} ${event.title} ${event.detail}`.toLowerCase().includes(searchValue),
      emptyLabel: "No events"
    });
    if (eventCountLabel) {
      eventCountLabel.textContent = typeof visible === "number" ? `${visible} events` : visible;
    }
  };

  jobSearchInput?.addEventListener("input", refreshJobFilters);
  jobStatusSelect?.addEventListener("change", refreshJobFilters);
  jobResetButton?.addEventListener("click", () => {
    if (jobSearchInput) jobSearchInput.value = "";
    if (jobStatusSelect) jobStatusSelect.value = "all";
    refreshJobFilters();
  });
  eventSearchInput?.addEventListener("input", refreshEventFilters);
  eventResetButton?.addEventListener("click", () => {
    if (eventSearchInput) eventSearchInput.value = "";
    refreshEventFilters();
  });
  refreshJobFilters();
  refreshEventFilters();

  return operationsView;
}
