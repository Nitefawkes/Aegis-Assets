# 🌐 Aegis-Assets REST API Documentation

The Aegis-Assets REST API provides programmatic access to the asset database and management functions. The API follows RESTful conventions and returns JSON responses.

## 🚀 Getting Started

### Starting the API Server

```bash
# Start with default settings (localhost:3000)
./target/release/aegis serve --features api

# Custom address and database
./target/release/aegis serve --address 0.0.0.0:8080 --database ./my-assets.db

# Disable CORS (default: enabled)
./target/release/aegis serve --cors false
```

### Base URL

```
http://localhost:3000/api/v1
```

## 📋 Core Endpoints

### Health Check

Check if the API server is running and get version information.

**GET** `/health`

```json
{
  "status": "healthy",
  "version": "0.2.0",
  "timestamp": 1694789122,
  "features": {
    "plugin_marketplace": true,
    "parallel_processing": false,
    "streaming_extraction": false,
    "advanced_memory_mgmt": false
  }
}
```

### API Information

Get API metadata and available endpoints.

**GET** `/info`

```json
{
  "name": "Aegis-Assets API",
  "version": "0.2.0",
  "description": "REST API for game asset extraction and management",
  "endpoints": [
    "/api/v1/health",
    "/api/v1/info",
    "/api/v1/assets",
    "/api/v1/assets/search",
    "/api/v1/assets/{id}",
    "/api/v1/assets/stats",
    "/api/v1/db/index",
    "/api/v1/db/stats",
    "/api/v1/plugins",
    "/api/v1/plugins/search",
    "/api/v1/plugins/{id}",
    "/api/v1/plugins/{id}/install",
    "/api/v1/plugins/{id}/versions",
    "/api/v1/plugins/stats",
    "/api/v1/extract",
    "/api/v1/extract/batch",
    "/api/v1/extract/progress/{id}"
  ]
}
```

## 📁 Asset Endpoints

### List Assets

Retrieve a list of assets with optional filtering.

**GET** `/assets`

**Query Parameters:**
- `asset_type` (optional) - Filter by asset type (Texture, Mesh, Audio, etc.)
- `limit` (optional) - Maximum number of results (default: 50, max: 1000)

**Example:**
```bash
GET /api/v1/assets?asset_type=Texture&limit=10
```

**Response:**
```json
{
  "assets": [
    {
      "id": "asset_123",
      "name": "character_texture.png",
      "asset_type": "Texture",
      "source_path": "/path/to/source.png",
      "output_path": "/path/to/output",
      "file_size": 2048576,
      "export_format": "PNG",
      "tags": ["character", "texture", "demo"],
      "description": "Main character texture",
      "created_at": 1694789122,
      "updated_at": 1694789122,
      "game_id": "my-game",
      "compliance_level": "Safe",
      "content_hash": "sha256:abc123..."
    }
  ],
  "total": 42,
  "limit": 10
}
```

### Search Assets

Search for assets using text queries and filters.

**GET** `/assets/search`

**Query Parameters:**
- `q` (optional) - Text search query
- `asset_type` (optional) - Filter by asset type
- `tags` (optional) - Filter by tags (comma-separated)
- `game` (optional) - Filter by game ID
- `compliance` (optional) - Filter by compliance level
- `limit` (optional) - Maximum results (default: 50)
- `sort_by` (optional) - Sort order (Relevance, NameAsc, NameDesc, CreatedAsc, CreatedDesc, SizeAsc, SizeDesc)

**Example:**
```bash
GET /api/v1/assets/search?q=character&asset_type=Texture&tags=hero,main
```

**Response:**
```json
{
  "results": [
    {
      "asset": {
        "id": "asset_123",
        "name": "hero_texture.png",
        "asset_type": "Texture",
        // ... full asset object
      },
      "relevance_score": 0.95,
      "matched_fields": ["name", "tags"]
    }
  ],
  "query": "character",
  "total": 3
}
```

### Get Asset by ID

Retrieve a specific asset by its ID.

**GET** `/assets/{id}`

**Response:**
```json
{
  "id": "asset_123",
  "name": "character_texture.png",
  "asset_type": "Texture",
  // ... full asset object
}
```

### Asset Statistics

Get database statistics and asset distribution.

**GET** `/assets/stats`

**Response:**
```json
{
  "total_assets": 1234,
  "total_size": 524288000,
  "assets_by_type": {
    "Texture": 456,
    "Mesh": 234,
    "Audio": 123,
    "Material": 89,
    "Level": 12
  },
  "tags": {
    "character": 145,
    "environment": 234,
    "ui": 67,
    "demo": 456
  }
}
```

## 🗄️ Database Endpoints

### Index Assets

Add assets from a directory to the database.

**POST** `/db/index`

**Request Body:**
```json
{
  "directory": "/path/to/assets",
  "game": "my-game-id",
  "tags": ["extracted", "demo", "v1.0"]
}
```

**Response:**
```json
{
  "message": "Successfully indexed 42 assets",
  "indexed_count": 42,
  "directory": "/path/to/assets"
}
```

### Database Statistics

Alias for `/assets/stats` - get database statistics.

**GET** `/db/stats`

## 🔌 Plugin Marketplace Endpoints

### List Available Plugins

Retrieve a list of plugins available in the marketplace.

**GET** `/plugins`

**Query Parameters:**
- `engine` (optional) - Filter by game engine (unity, unreal, godot, generic)
- `risk_level` (optional) - Filter by risk level (low, medium, high)
- `limit` (optional) - Maximum number of results (default: 50, max: 100)
- `offset` (optional) - Result offset for pagination (default: 0)

**Response:**
```json
{
  "plugins": [
    {
      "id": "unity-asset-extractor",
      "name": "Unity Asset Extractor",
      "version": "2.1.0",
      "description": "Extract and convert assets from Unity game files",
      "author": "Aegis Team",
      "engine": "unity",
      "risk_level": "low",
      "publisher_policy": "permissive",
      "tags": ["unity", "assets", "extraction"],
      "downloads": 15420,
      "rating": 4.8,
      "last_updated": "2024-01-15T10:30:00Z",
      "installed": false,
      "has_update": false
    }
  ],
  "total": 156,
  "limit": 50,
  "offset": 0
}
```

### Search Plugins

Search for plugins using text queries and filters.

**GET** `/plugins/search`

**Query Parameters:**
- `q` (optional) - Text search query
- `engine` (optional) - Filter by game engine
- `risk_level` (optional) - Filter by risk level
- `tags` (optional) - Filter by tags (comma-separated)
- `sort_by` (optional) - Sort order (relevance, popularity, updated, name)
- `limit` (optional) - Maximum results (default: 20)

**Response:**
```json
{
  "results": [
    {
      "plugin": {
        "id": "unity-asset-extractor",
        "name": "Unity Asset Extractor",
        // ... full plugin object
      },
      "relevance_score": 0.95,
      "matched_fields": ["name", "description", "tags"]
    }
  ],
  "query": "unity extractor",
  "total": 12
}
```

### Get Plugin Details

Retrieve detailed information about a specific plugin.

**GET** `/plugins/{id}`

**Response:**
```json
{
  "plugin": {
    "id": "unity-asset-extractor",
    "name": "Unity Asset Extractor",
    "version": "2.1.0",
    "description": "Complete Unity asset extraction and conversion toolkit",
    "author": "Aegis Team",
    "license": "MIT",
    "homepage": "https://github.com/aegis-assets/unity-extractor",
    "repository": "https://github.com/aegis-assets/unity-extractor",
    "keywords": ["unity", "assets", "extraction", "converter"],
    "engine": "unity",
    "risk_level": "low",
    "publisher_policy": "permissive",
    "bounty_eligible": true,
    "enterprise_approved": true,
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-15T10:30:00Z",
    "status": "approved"
  },
  "versions": ["2.1.0", "2.0.1", "2.0.0", "1.5.2"],
  "download_stats": {
    "total_downloads": 15420,
    "downloads_last_30_days": 1234,
    "version_breakdown": {
      "2.1.0": 5678,
      "2.0.1": 4321
    }
  },
  "rating": {
    "average": 4.8,
    "count": 234,
    "distribution": {
      "5": 189,
      "4": 34,
      "3": 8,
      "2": 2,
      "1": 1
    }
  }
}
```

### Install Plugin

Install or update a plugin from the marketplace.

**POST** `/plugins/{id}/install`

**Query Parameters:**
- `version` (optional) - Specific version to install (default: latest)

**Request Body:**
```json
{
  "force": false,
  "skip_dependencies": false
}
```

**Response:**
```json
{
  "message": "Successfully installed unity-asset-extractor v2.1.0",
  "plugin_id": "unity-asset-extractor",
  "version": "2.1.0",
  "installation_time_ms": 15420,
  "dependencies_installed": ["serde", "tokio"]
}
```

### Get Plugin Versions

Retrieve version history for a plugin.

**GET** `/plugins/{id}/versions`

**Response:**
```json
{
  "plugin_id": "unity-asset-extractor",
  "versions": [
    {
      "version": "2.1.0",
      "published_at": "2024-01-15T10:30:00Z",
      "package_size": 2457600,
      "package_hash": "sha256:abc123...",
      "changelog": "Added support for Unity 2023...",
      "breaking_changes": false,
      "download_count": 5678
    },
    {
      "version": "2.0.1",
      "published_at": "2024-01-10T14:20:00Z",
      "package_size": 2345678,
      "package_hash": "sha256:def456...",
      "changelog": "Bug fixes and performance improvements",
      "breaking_changes": false,
      "download_count": 4321
    }
  ]
}
```

### Plugin Marketplace Statistics

Get marketplace-wide statistics and analytics.

**GET** `/plugins/stats`

**Response:**
```json
{
  "total_plugins": 156,
  "total_versions": 432,
  "total_downloads": 45678,
  "pending_reviews": 23,
  "plugins_by_engine": {
    "unity": 67,
    "unreal": 34,
    "godot": 23,
    "generic": 32
  },
  "plugins_by_risk_level": {
    "low": 123,
    "medium": 28,
    "high": 5
  },
  "top_downloads": [
    {
      "plugin_id": "unity-asset-extractor",
      "name": "Unity Asset Extractor",
      "downloads": 15420
    }
  ],
  "recent_updates": [
    {
      "plugin_id": "unreal-texture-tools",
      "name": "Unreal Texture Tools",
      "version": "1.5.2",
      "updated_at": "2024-01-10T14:20:00Z"
    }
  ]
}
```

## ⚡ Enhanced Extraction Endpoints

### Single File Extraction

Extract assets from a single file with advanced options.

**POST** `/extract`

**Request Body:**
```json
{
  "source_path": "/path/to/game.unity3d",
  "output_dir": "/path/to/output/",
  "options": {
    "convert_assets": true,
    "max_memory_mb": 4096,
    "enable_streaming": false,
    "parallel_processing": false,
    "quality_settings": {
      "texture_format": "PNG",
      "mesh_format": "GLTF",
      "audio_format": "OGG",
      "compression_level": "high"
    },
    "compliance_mode": "strict"
  }
}
```

**Response:**
```json
{
  "extraction_id": "extract_12345",
  "source_path": "/path/to/game.unity3d",
  "status": "processing",
  "progress": 0.0,
  "estimated_completion": "2024-01-15T10:35:00Z",
  "resources_found": 0,
  "bytes_processed": 0
}
```

### Batch Extraction

Extract assets from multiple files concurrently.

**POST** `/extract/batch`

**Request Body:**
```json
{
  "sources": [
    "/path/to/game1.unity3d",
    "/path/to/game2.pak",
    "/path/to/level1.dat"
  ],
  "output_dir": "/path/to/output/",
  "options": {
    "max_concurrent": 4,
    "continue_on_error": true,
    "progress_reporting": true
  }
}
```

**Response:**
```json
{
  "batch_id": "batch_67890",
  "total_files": 3,
  "status": "processing",
  "extractions": [
    {
      "extraction_id": "extract_12345",
      "source_path": "/path/to/game1.unity3d",
      "status": "processing",
      "progress": 0.25
    }
  ]
}
```

### Extraction Progress

Monitor the progress of an ongoing extraction.

**GET** `/extract/progress/{id}`

**Response:**
```json
{
  "extraction_id": "extract_12345",
  "status": "processing",
  "progress": 0.75,
  "stage": "converting_textures",
  "resources_extracted": 23,
  "resources_total": 31,
  "bytes_processed": 24567890,
  "bytes_total": 32768000,
  "estimated_time_remaining_ms": 45000,
  "performance_metrics": {
    "processing_speed_mbps": 45.2,
    "memory_usage_mb": 234,
    "compression_ratio": 0.85
  },
  "warnings": [
    "Large texture detected: main_character_diffuse.png (8MB)"
  ]
}
```

## 🔍 Asset Types

The API supports the following asset types:

- `Texture` - Images, sprites, textures
- `Mesh` - 3D models, geometry
- `Audio` - Sound effects, music
- `Material` - Shaders, material definitions
- `Level` - Game levels, scenes
- `Animation` - Animation clips, sequences
- `Script` - Code files, scripts
- `Other` - Miscellaneous files

## 📊 Sort Orders

Available sort orders for search results:

- `Relevance` - By search relevance (default for search)
- `NameAsc` - By name (A-Z)
- `NameDesc` - By name (Z-A)
- `CreatedAsc` - By creation date (oldest first)
- `CreatedDesc` - By creation date (newest first)
- `SizeAsc` - By file size (smallest first)
- `SizeDesc` - By file size (largest first)

## 🚨 Error Handling

The API returns appropriate HTTP status codes and JSON error messages:

**400 Bad Request** - Invalid parameters
```json
{
  "error": "Invalid asset type specified",
  "status": 400
}
```

**404 Not Found** - Resource not found
```json
{
  "error": "Asset not found",
  "status": 404
}
```

**500 Internal Server Error** - Server error
```json
{
  "error": "Database connection failed",
  "status": 500
}
```

## 🌐 CORS Support

The API includes CORS headers by default to support web applications:

```
Access-Control-Allow-Origin: *
Access-Control-Expose-Headers: *
Vary: origin, access-control-request-method, access-control-request-headers
```

## 🛡️ Security Considerations

**Current State (v0.2.0):**
- No authentication required
- Rate limiting not implemented
- Intended for local/development use

**Planned Features:**
- API key authentication
- Rate limiting per client
- Role-based access control
- Request logging and monitoring

## 📚 Examples

### Python Client

```python
import requests

# Search for textures
response = requests.get('http://localhost:3000/api/v1/assets/search', 
                       params={'q': 'character', 'asset_type': 'Texture'})
assets = response.json()

for result in assets['results']:
    asset = result['asset']
    print(f"Found: {asset['name']} (score: {result['relevance_score']})")
```

### JavaScript Client

```javascript
// Get asset statistics
fetch('http://localhost:3000/api/v1/assets/stats')
  .then(response => response.json())
  .then(stats => {
    console.log(`Total assets: ${stats.total_assets}`);
    console.log('By type:', stats.assets_by_type);
  });
```

### cURL Examples

```bash
# Health check
curl http://localhost:3000/api/v1/health

# Search assets
curl "http://localhost:3000/api/v1/assets/search?q=texture&limit=5"

# Index new assets
curl -X POST http://localhost:3000/api/v1/db/index \
  -H "Content-Type: application/json" \
  -d '{"directory": "./my-assets", "game": "test-game", "tags": ["demo"]}'
```

## 🔮 Future Enhancements

- GraphQL endpoint for complex queries
- WebSocket support for real-time updates
- Batch operations for multiple assets
- Asset thumbnail generation and serving
- Export capabilities (ZIP, TAR)
- Advanced filtering and faceted search
