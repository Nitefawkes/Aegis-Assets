import { createElement } from "../utils/dom.js";

function galleryCard(asset) {
  return createElement("div", {
    className: "gallery__card",
    children: [
      createElement("div", {
        className: "gallery__preview",
        text: asset.previewLabel
      }),
      createElement("strong", { text: asset.name }),
      createElement("p", { text: asset.description }),
      createElement("div", {
        className: "tag",
        text: `Provenance: ${asset.provenance}`
      })
    ]
  });
}

export function AssetGallery({ data }) {
  const assets = data.assets || [];
  return createElement("section", {
    className: "card",
    children: [
      createElement("h3", { text: "Asset Gallery" }),
      createElement("div", {
        className: "gallery",
        children: assets.map(galleryCard)
      })
    ]
  });
}
