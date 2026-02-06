import { createElement } from "../utils/dom.js";

function policyNode(node) {
  return createElement("div", {
    className: "policy-node",
    children: [
      createElement("h4", { text: node.name }),
      createElement("p", { text: node.description }),
      createElement("div", {
        className: "tag",
        text: `Controls: ${node.controls}`
      })
    ]
  });
}

export function PolicyStudio({ data }) {
  const profiles = data.profiles || [];

  return createElement("div", {
    className: "card-grid",
    children: profiles.map((profile) =>
      createElement("section", {
        className: "card",
        children: [
          createElement("div", {
            className: "page-header",
            children: [
              createElement("div", {
                children: [
                  createElement("h3", { text: profile.name }),
                  createElement("p", { text: profile.summary })
                ]
              }),
              createElement("span", { className: "tag", text: profile.owner })
            ]
          }),
          createElement("div", {
            className: "policy-canvas",
            children: profile.nodes.map(policyNode)
          })
        ]
      })
    )
  });
}
