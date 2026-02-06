import { createElement } from "../utils/dom.js";

function buildHeatmapCell({ title, score, status, description }) {
  return createElement("div", {
    className: "heatmap__cell",
    children: [
      createElement("span", { className: "tag", text: status }),
      createElement("strong", { text: `${score}%` }),
      createElement("span", { text: title }),
      createElement("small", { text: description })
    ]
  });
}

export function RiskHeatmap({ data }) {
  const grids = [
    { key: "profiles", label: "Profiles" },
    { key: "projects", label: "Projects" },
    { key: "publishers", label: "Publishers" }
  ];

  const gridElements = grids.map((grid) => {
    const entries = data[grid.key] || [];
    return createElement("section", {
      className: "card",
      children: [
        createElement("h3", { text: `Risk by ${grid.label}` }),
        createElement("div", {
          className: "heatmap",
          children: entries.map(buildHeatmapCell)
        })
      ]
    });
  });

  return createElement("div", {
    className: "card-grid",
    children: gridElements
  });
}
