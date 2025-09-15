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
  "timestamp": 1694789122
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
    "/api/v1/db/stats"
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
