import { createElement, formatTimestamp } from "../utils/dom.js";

export function OperationsConsole({ data, onSubmitJob, onRefresh }) {
  const statusItems = data.statusMessages.map((message) =>
    createElement("li", { text: message })
  );

  const jobItems = data.jobs.length
    ? data.jobs.map((job) =>
        createElement("li", {
          className: "job-card",
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
          ]
        })
      )
    : [createElement("li", { className: "empty-state", text: "No jobs submitted yet." })];

  const eventItems = data.events.length
    ? data.events.map((event) =>
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
    : [createElement("li", { className: "empty-state", text: "No events yet." })];

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
    const payload = {
      source_path: formData.get("source_path"),
      output_dir: formData.get("output_dir")
    };
    await onSubmitJob?.(payload);
    jobForm.reset();
  });

  return createElement("div", {
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
      createElement("div", {
        className: "card",
        children: [
          createElement("h3", { text: "Recent jobs" }),
          createElement("ul", { className: "event-list", children: jobItems })
        ]
      }),
      complianceCard,
      createElement("div", {
        className: "card",
        children: [
          createElement("h3", { text: "Live event stream" }),
          createElement("ul", { className: "event-list", children: eventItems })
        ]
      })
    ]
  });
}
