# 🛡️ Aegis-Assets

**Compliance-first platform for game asset extraction, preservation, and creative workflows.**

Aegis-Assets is not *just another extractor*. It's the first professional-grade, compliance-aware system built for:

* 🎨 **Modders & creators** → unlock game assets for personal projects & content workflows.
* 🏛️ **Archivists & researchers** → preserve digital culture safely and sustainably.
* 🏢 **Studios & institutions** → integrate asset pipelines with risk management built in.

## 🚀 Mission

**To unify fragmented extraction tools into sustainable infrastructure for the next generation of game-adjacent creative industries.** Where others chase raw breadth, Aegis competes on **trust, compliance, and sustainability**.

## ⚖️ The Compliance Manifesto

Aegis-Assets is built on a **compliance-first architecture**. That means we **do not support every format at any cost**. Instead, we:

### 1. **Respect Legal Boundaries**
* If a publisher has a history of aggressive IP enforcement (e.g. Nintendo), we **do not provide official support**.
* These formats are excluded from bounties, documentation, and tutorials.

### 2. **Enable but Don't Endorse**
* The plugin system allows community submissions.
* High-risk formats are clearly **sandboxed with disclaimers**.
* Enterprises can mark formats as "high-risk" via compliance profiles.

### 3. **Build for the Long Game**
* Compliance profiles make risks visible.
* Audit trails ensure institutional defensibility.
* Responsible choices today protect the community tomorrow.

## 🛠️ Features

* ⚡ **Rust Core Engine** → high-performance, memory-safe, parallel extraction.
* 🔌 **Plugin Architecture** → extendable format support without burning out maintainers.
* 🗂️ **Patch Recipe System** → exports deltas, not copyrighted content.
* 📜 **Compliance Profiles** → per-game legal risk indicators.
* 📝 **Audit Trails** → enterprise-ready provenance logs.
* 🤖 **AI-Powered Tools (Pro)** → auto-tagging, PBR derivation, semantic search.
* 🖼️ **GUI & Web Preview** → glTF2 + KTX2 + OGG viewing without engines installed.

> ℹ️ **Current implementation snapshot:** Several of the above achievements are still prototypes or placeholders in the codebase. See [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) for a detailed review of what is implemented today and the immediate tasks required to reach these goals.

## 🔒 Why Compliance Matters

Every other tool competes on format breadth. Aegis competes on **trust**:

* For **enterprises**: built-in risk management.
* For **communities**: sustainable, future-proof development.
* For **investors/partners**: defensible moat through compliance leadership.

This stance is not a weakness — it's our **category-defining advantage**.

## 🌐 Roadmap (Phased)

**Phase 1 (0–6 months):**
* Unity & Unreal baseline support.
* Patch recipes default.
* Compliance manifesto published.

**Phase 2 (6–12 months):**
* AI tagging (Rekognition/CLIP).
* Plugin marketplace + bounty board.
* Enterprise pilots w/ archives & labs.

**Phase 3 (12–18 months):**
* AI creator tools (auto-PBR, upscaling, LOD).
* Compliance dashboards for Pro users.
* Enterprise edition (audit logs, Steam/Epic checks).

## 👥 Community

* 💬 Join us on **Discord** to shape the roadmap.
* 🛠️ Contribute plugins via `aegis plugin new`.
* 🏆 Recognition for plugin pioneers in marketplace & docs.

**Contributor Principles:**
* Respect compliance guidelines.
* No redistribution of copyrighted assets.
* Build for sustainability, not short-term hacks.

## 🚀 Quick Start

```bash
# Install Aegis-Assets
cargo install aegis-assets

# Extract Unity assets (with compliance checks)
aegis extract --engine unity --input game.unity3d --output ./assets/

# Create patch recipe (distributable, legal)
aegis recipe create --source game.unity3d --assets ./assets/ --output game-assets.recipe

# Apply patch recipe (requires original game files)
aegis recipe apply --recipe game-assets.recipe --source game.unity3d --output ./extracted/
```

## 📜 License & Legal

* **Core engine:** Apache-2.0.
* **Plugins/examples:** MIT.
* **Disclaimer:** Aegis-Assets extracts only from **legally owned game files**.
  * Exports are for **personal use, research, and preservation**.
  * No copyrighted content is redistributed.

## 🌟 Positioning

Aegis-Assets = **Professional infrastructure** for game asset workflows.

* Not a "hobby extractor."
* Not a "grey-area ripper."
* A **compliance-driven platform** trusted by creators, institutions, and enterprises.

---

**Compliance isn't a constraint—it's our competitive advantage.**
