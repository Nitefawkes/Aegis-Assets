import test from "node:test";
import assert from "node:assert/strict";

import { RiskHeatmap } from "../src/components/RiskHeatmap.js";
import { JobTimeline } from "../src/components/JobTimeline.js";
import { PolicyStudio } from "../src/components/PolicyStudio.js";
import { AssetGallery } from "../src/components/AssetGallery.js";
import { AuditTimeline } from "../src/components/AuditTimeline.js";

function createMockElement(tagName) {
  return {
    tagName: tagName.toUpperCase(),
    children: [],
    attributes: {},
    className: "",
    textContent: "",
    appendChild(child) {
      this.children.push(child);
      return child;
    },
    setAttribute(key, value) {
      if (value !== null) {
        this.attributes[key] = value;
      }
    },
    addEventListener() {}
  };
}

function withDom(callback) {
  global.document = {
    createElement: (tagName) => createMockElement(tagName)
  };
  callback();
  delete global.document;
}

const sampleData = {
  risk: { profiles: [], projects: [], publishers: [] },
  jobs: { jobs: [] },
  policies: { profiles: [] },
  assets: { assets: [] },
  audit: { events: [], filters: { query: "", categories: ["All"], actors: ["All"] } }
};

test("components render without crashing", () => {
  withDom(() => {
    assert.equal(RiskHeatmap({ data: sampleData.risk }).tagName, "DIV");
    assert.equal(JobTimeline({ data: sampleData.jobs }).tagName, "DIV");
    assert.equal(PolicyStudio({ data: sampleData.policies }).tagName, "DIV");
    assert.equal(AssetGallery({ data: sampleData.assets }).tagName, "SECTION");
    assert.equal(
      AuditTimeline({ data: sampleData.audit, onFilterChange: () => {} }).tagName,
      "SECTION"
    );
  });
});
