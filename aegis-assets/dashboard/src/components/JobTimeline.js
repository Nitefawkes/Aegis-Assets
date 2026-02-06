import { createElement, formatTimestamp } from "../utils/dom.js";

function timelineItem(step) {
  return createElement("div", {
    className: "timeline__item",
    children: [
      createElement("div", { className: "timeline__dot" }),
      createElement("div", {
        className: "timeline__content",
        children: [
          createElement("strong", { text: step.title }),
          createElement("p", { text: step.detail }),
          createElement("span", { className: "tag", text: formatTimestamp(step.timestamp) })
        ]
      })
    ]
  });
}

export function JobTimeline({ data }) {
  const jobs = data.jobs || [];
  return createElement("div", {
    className: "card-grid",
    children: jobs.map((job) =>
      createElement("section", {
        className: "card",
        children: [
          createElement("div", {
            className: "page-header",
            children: [
              createElement("div", {
                children: [
                  createElement("h3", { text: job.name }),
                  createElement("p", { text: job.status })
                ]
              }),
              createElement("span", { className: "tag", text: job.owner })
            ]
          }),
          createElement("div", {
            className: "timeline",
            children: job.timeline.map(timelineItem)
          })
        ]
      })
    )
  });
}
