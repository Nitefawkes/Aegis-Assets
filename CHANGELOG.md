# Changelog

All notable changes to Aegis-Assets will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2025-09-15

### 🚀 Major Features Added

#### **Asset Database & Search System**
- Complete SQLite-based asset database with metadata storage
- Full-text search with relevance scoring and ranking
- Asset type filtering (Texture, Mesh, Audio, Material, Level, etc.)
- Tag-based organization and categorization
- Game ID association for multi-project management
- Comprehensive statistics and analytics

#### **REST API Server**
- Complete HTTP API built with Axum framework
- JSON responses for all endpoints with proper error handling
- CORS support for web integration
- Asset listing, searching, and retrieval endpoints
- Database management and indexing endpoints
- Health monitoring and API information endpoints

#### **Web Dashboard Interface**
- Modern, responsive browser-based interface
- Real-time asset statistics and analytics
- Advanced search with live results
- Asset browsing with type and tag filtering
- Beautiful UI with gradients and animations
- API endpoint documentation built-in

#### **Enhanced Unity Plugin**
- Complete UnityFS format parser supporting Unity 3.x through 2023.x
- Comprehensive texture decompression (DXT1, DXT5, ETC1, ETC2, ASTC)
- PNG export for textures with proper RGBA/RGB conversion
- glTF 2.0 mesh export with positions, normals, UVs, and indices
- Robust compression handling (LZ4, LZMA, LZ4HC)
- Graceful fallback for unknown compression types
- Asset categorization and metadata extraction

#### **Unreal Engine Plugin Foundation**
- Basic PAK file detection and parsing
- UAsset file format support (foundation)
- IoStore support framework
- Placeholder implementations for future enhancement

#### **Asset Conversion Pipeline**
- PNG conversion for multiple texture formats
- glTF 2.0 export with binary data generation
- Base64 encoding for embedded resources
- Compression statistics and reporting
- Memory-mapped file handling for large assets

### 🔧 Technical Improvements

#### **Core Architecture**
- Plugin registry with automatic discovery
- Improved error handling with detailed error types
- Parallel processing for asset extraction
- Memory-efficient streaming for large files
- Comprehensive logging and tracing

#### **CLI Enhancements**
- New `db` subcommand for database operations
- `serve` subcommand to start API server
- `plugins` command to list available formats
- Asset indexing with directory scanning
- Conversion pipeline integration

#### **Developer Experience**
- Feature-gated API compilation (`--features api`)
- Comprehensive testing with real Unity files
- Mock data generation for testing
- Integration tests for end-to-end workflows
- Improved documentation and examples

### 🐛 Bug Fixes
- Fixed Unity file detection for serialized files with zero headers
- Resolved LZMA decompression size mismatch issues
- Fixed compilation errors in asset conversion pipeline
- Corrected PowerShell command syntax in build scripts
- Resolved Cargo workspace dependency conflicts

### 📚 Documentation
- Updated README with current feature set
- Added web dashboard usage instructions
- Enhanced quick start guide with API examples
- Updated roadmap to reflect completed priorities
- Added comprehensive feature matrix

### 🔄 Breaking Changes
- Asset database schema introduced (requires reindexing)
- API endpoints follow new REST conventions
- CLI commands restructured with subcommands

## [0.1.0] - 2025-08-25

### Added
- Initial implementation of Aegis-Assets compliance-first extraction platform
- Core Rust library with plugin architecture
- Unity plugin with basic format detection and extraction
- Python bindings via PyO3
- Command-line interface with extract, recipe, and compliance commands
- Compliance framework with publisher risk profiles
- Patch recipe system for legal asset distribution
- CI/CD pipeline with multi-platform builds
- Comprehensive documentation and contributor guidelines

### Changed
- N/A (initial release)

### Deprecated
- N/A (initial release)

### Removed
- N/A (initial release)

### Fixed
- N/A (initial release)

### Security
- Built-in compliance checking to prevent high-risk extractions
- Audit trail system for enterprise users
- No direct asset redistribution (patch recipe model only)

## [0.1.0] - TBD

### Added
- Initial public release
- MVP feature set as outlined in strategic roadmap
- Unity plugin with basic functionality
- Compliance profiles for major publishers (Bethesda, Nintendo, Valve, etc.)
- Professional documentation and legal framework
