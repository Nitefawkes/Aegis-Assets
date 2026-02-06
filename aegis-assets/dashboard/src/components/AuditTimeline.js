import { createElement, formatTimestamp } from "../utils/dom.js";

function auditItem(event) {
  return createElement("div", {
    className: "timeline__item",
    children: [
      createElement("div", { className: "timeline__dot" }),
      createElement("div", {
        className: "timeline__content",
        children: [
          createElement("strong", { text: event.title }),
          createElement("p", { text: event.detail }),
          createElement("div", {
            className: "filters",
            children: [
              createElement("span", { className: "tag", text: event.actor }),
              createElement("span", { className: "tag", text: event.category }),
              createElement("span", { className: "tag", text: formatTimestamp(event.timestamp) })
            ]
          })
        ]
      })
    ]
  });
}

function buildSelect({ name, options, value }) {
  return createElement("select", {
    attributes: { name },
    children: options.map((option) =>
      createElement("option", {
        text: option,
        attributes: { value: option, selected: option === value ? "selected" : null }
      })
    )
  });
}

export function AuditTimeline({ data, onFilterChange }) {
  const events = data.events || [];
  const filters = data.filters || { query: "", categories: ["All"], actors: ["All"] };

  const searchInput = createElement("input", {
    attributes: {
      type: "search",
      placeholder: "Search by asset, policy, or job",
      value: filters.query || ""
    }
  });

  const categorySelect = buildSelect({
    name: "category",
    options: filters.categories || ["All"],
    value: filters.category || "All"
  });
  const actorSelect = buildSelect({
    name: "actor",
    options: filters.actors || ["All"],
    value: filters.actor || "All"
  });

  const filterBar = createElement("div", {
    className: "filters",
    children: [searchInput, categorySelect, actorSelect]
  });

  searchInput.addEventListener("input", (event) => onFilterChange({ query: event.target.value }));
  [categorySelect, actorSelect].forEach((select) => {
    select.addEventListener("change", (event) => onFilterChange({ [event.target.name]: event.target.value }));
  });

  return createElement("section", {
    className: "card",
    children: [
      createElement("h3", { text: "Audit Timeline" }),
      filterBar,
      createElement("div", {
        className: "timeline",
        children: events.map(auditItem)
      })
    ]
  });
}
