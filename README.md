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

### ✅ **Core Engine (Implemented)**
* ⚡ **Rust Core Engine** → high-performance, memory-safe extraction framework with async architecture
* 🔌 **Plugin Architecture** → extensible format support with community marketplace and security validation
* 📦 **Asset Database** → SQLite-based storage with full-text search and metadata indexing
* 🔍 **Smart Search** → relevance scoring, tag filtering, type-based queries
* 🔐 **Security Framework** → plugin validation, threat assessment, and enterprise compliance
* 🌐 **REST API** → complete HTTP API with JSON responses for programmatic access
* 🎨 **Web Dashboard** → modern, responsive browser interface for asset management

### ✅ **Game Engine Support (Implemented)**
* 🎮 **Unity Engine** → UnityFS archives, serialized files, comprehensive asset extraction
  * 🖼️ **Textures**: PNG/KTX2 export with DXT/ETC/ASTC decompression, mipmap & alpha handling
  * 🎵 **Audio**: FSB4/FSB5 → WAV/OGG with Vorbis + Firelight ADPCM (GCADPCM/FADPCM) decoding, loop metadata preservation
  * 🔷 **Meshes**: glTF 2.0 export with OBJ fallback, complete pipeline with material support
* 🏛️ **Unreal Engine** → PAK files, UAsset parsing, IoStore support (foundation)
* 🏪 **Plugin Marketplace** → discover, install, and manage community plugins

### ✅ **Plugin Ecosystem (Complete)**
* 🔌 **Plugin Registry** → centralized marketplace with 8+ database tables
* 📋 **Manifest System** → TOML-based plugin specification and validation
* 🔗 **Dependency Resolution** → semantic versioning and conflict resolution
* 🖥️ **CLI Management** → install, uninstall, update, search plugins
* 🌐 **Web Marketplace** → beautiful interface for plugin discovery and management

### 🚧 **In Development**
* ⚡ **Performance Engine** → parallel processing, streaming, memory optimization
* 🧪 **Testing Framework** → comprehensive plugin and extraction validation  
* 🗜️ **Advanced Compression** → Oodle decompression bridge for modern game formats
* 📊 **Analytics Engine** → usage tracking, performance monitoring, optimization

## 🔒 Why Compliance Matters

Every other tool competes on format breadth. Aegis competes on **trust**:

* For **enterprises**: built-in risk management.
* For **communities**: sustainable, future-proof development.
* For **investors/partners**: defensible moat through compliance leadership.

This stance is not a weakness — it's our **category-defining advantage**.

## 🌐 Roadmap (Updated Progress)

### ✅ **Phase 1 (COMPLETED)**
* ✅ Unity & Unreal baseline support
* ✅ Core extraction pipeline with asset conversion
* ✅ REST API and web dashboard
* ✅ Asset database with search capabilities
* ✅ Compliance manifesto published
* ✅ **Plugin marketplace foundation** (database, CLI, web interface)

### 🚧 **Phase 2 (Current - Q4 2025):**
* ✅ **Sprint 1-2**: Unity texture pipeline (PNG/KTX2, atlas extraction, golden tests)
* ✅ **Sprint 3**: Unity audio pipeline (FSB decode, Vorbis/ADPCM, loop metadata, validation)
* ✅ **Sprint 3**: Unity mesh pipeline (glTF/OBJ export, materials, validation) — **COMPLETED**
* ✅ **Sprint 3**: Security framework integration (plugin validation, threat assessment, enterprise compliance) — **COMPLETED**
* 🔄 **Sprint 4**: Advanced compression (Oodle bridge, streaming decompression)
* 🔄 **Sprint 5**: Ethical sourcing integration (OpenGameArt, Itch.io, license detection)

### 🎯 **Phase 3 (12–18 months):**
* 🤖 AI tagging integration (CLIP/Rekognition)
* 🏢 Enterprise pilots with archives & museums
* 🎨 Advanced format support (Source Engine, Bethesda, mobile engines)
* 📊 Analytics engine & performance monitoring

### 🚀 **Phase 4 (18–24 months):**
* AI creator tools (auto-PBR, upscaling, LOD generation)
* Compliance dashboards for Pro users
* Enterprise edition (audit logs, Steam/Epic integration)
* Desktop GUI application with drag-and-drop

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
# Build from source (currently required)
git clone https://github.com/Nitefawkes/Aegis-Assets.git
cd aegis-assets
cargo build --release

# Extract Unity assets
./target/release/aegis extract --input game.unity3d --output ./assets/ --convert

# Plugin marketplace commands
./target/release/aegis plugins search "unity extractor"
./target/release/aegis plugins install unity-asset-extractor
./target/release/aegis plugins list --verbose
./target/release/aegis plugins update

# Start the web API server
./target/release/aegis serve --features api

# Use the asset database
./target/release/aegis db index ./assets/ --game "MyGame" --tags demo,extracted
./target/release/aegis db search "texture" --asset-type Texture
./target/release/aegis db stats
```

### ⚠️ Python bindings status

The `aegis-python` crate currently provides a stub PyO3 module. Functional bindings for configuring and running extractions from Python are still in development, so all workflows should use the Rust CLI or core library for the time being.

### 🌐 **Web Dashboard**

After starting the API server, access the web interfaces:

**Main Dashboard** (`index.html`):
- 📊 Real-time asset statistics and system status
- 🔍 Advanced search interface with filtering
- 📁 Asset browsing and preview
- 🔌 Plugin management and installation

**Plugin Marketplace** (`marketplace.html`):
- 🏪 Discover and install community plugins
- 🔍 Search by engine, risk level, and features
- 📦 Manage installed plugins and updates
- 📊 Marketplace analytics and security overview

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
