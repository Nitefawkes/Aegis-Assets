# Asset Metadata Schema v1.0 - Phase 2 Sprint 1

## Schema Overview

The metadata schema defines how AI-generated tags, provenance data, and asset attributes are structured for storage, search, and compliance tracking.

## Core Principles

1. **Backward Compatibility**: Must work with existing Unity plugin data
2. **Extensibility**: Support future asset types and engines  
3. **Compliance-First**: Built-in audit trails and consent tracking
4. **Performance**: Optimized for search and filtering operations

## Database Schema

### Primary Tables

#### `assets` (Core Asset Registry)
```sql
CREATE TABLE assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id VARCHAR(255) NOT NULL,           -- From Unity plugin (e.g., "object_12345")
    source_file_path TEXT NOT NULL,           -- Original file path
    source_file_hash CHAR(64) NOT NULL,       -- Blake3 hash
    asset_name VARCHAR(255),                  -- Display name
    file_type VARCHAR(50),                    -- "Texture2D", "Mesh", "AudioClip"
    size_bytes BIGINT,                        -- Uncompressed size
    extraction_time TIMESTAMPTZ DEFAULT NOW(),
    compliance_profile_id UUID REFERENCES compliance_profiles(id),
    provenance JSONB,                         -- Provenance data from Unity plugin
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_assets_source_hash ON assets(source_file_hash);
CREATE INDEX idx_assets_file_type ON assets(file_type);
CREATE INDEX idx_assets_extraction_time ON assets(extraction_time);
```

#### `ai_tags` (AI-Generated Semantic Tags)
```sql
CREATE TABLE ai_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id UUID REFERENCES assets(id) ON DELETE CASCADE,
    tag_name VARCHAR(100) NOT NULL,           -- "character", "environment", "weapon"
    confidence DECIMAL(4,3) CHECK (confidence >= 0 AND confidence <= 1),
    tag_category VARCHAR(50),                 -- "object_type", "art_style", "technical"
    ai_model VARCHAR(100),                    -- "clip-vit-b32", "aws-rekognition-v2"
    ai_model_version VARCHAR(50),             -- Version tracking
    processing_time_ms INTEGER,               -- Performance tracking
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(asset_id, tag_name, ai_model)      -- Prevent duplicate tags per model
);

CREATE INDEX idx_ai_tags_asset_id ON ai_tags(asset_id);
CREATE INDEX idx_ai_tags_tag_name ON ai_tags(tag_name);
CREATE INDEX idx_ai_tags_confidence ON ai_tags(confidence);
CREATE INDEX idx_ai_tags_category ON ai_tags(tag_category);
```

#### `asset_attributes` (Technical Properties)
```sql
CREATE TABLE asset_attributes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id UUID REFERENCES assets(id) ON DELETE CASCADE,
    attribute_name VARCHAR(100) NOT NULL,     -- "width", "polygon_count", "duration_seconds"
    attribute_value TEXT,                     -- Stored as text, cast as needed
    attribute_type VARCHAR(20) DEFAULT 'string', -- "integer", "decimal", "boolean", "string"
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(asset_id, attribute_name)
);

CREATE INDEX idx_asset_attributes_asset_id ON asset_attributes(asset_id);
CREATE INDEX idx_asset_attributes_name ON asset_attributes(attribute_name);
```

#### `ai_processing_consent` (Compliance Tracking)
```sql
CREATE TABLE ai_processing_consent (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id UUID REFERENCES assets(id) ON DELETE CASCADE,
    user_id VARCHAR(255),                     -- Enterprise user identifier
    consent_given BOOLEAN NOT NULL,
    consent_scope TEXT[],                     -- ["tagging", "cloud_processing", "model_training"]
    ai_models_authorized TEXT[],              -- ["clip-local", "aws-rekognition"]
    consent_timestamp TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ,                   -- Optional expiration
    
    UNIQUE(asset_id, user_id)
);

CREATE INDEX idx_consent_asset_id ON ai_processing_consent(asset_id);
CREATE INDEX idx_consent_user_id ON ai_processing_consent(user_id);
```

## Tag Taxonomy (20+ Categories)

### Object Types
- `character` - Human/creature models
- `environment` - Landscapes, buildings, terrain
- `weapon` - Guns, swords, projectiles
- `vehicle` - Cars, ships, aircraft
- `prop` - Furniture, decorations, items
- `ui_element` - Buttons, panels, icons

### Art Styles  
- `realistic` - Photorealistic rendering
- `stylized` - Cartoon/artistic style
- `pixel_art` - Retro pixel graphics
- `low_poly` - Minimalist geometry
- `anime` - Japanese animation style
- `cyberpunk` - Futuristic aesthetic

### Technical Attributes
- `high_poly` - >10k polygons
- `low_poly` - <1k polygons
- `animated` - Contains animation data
- `rigged` - Has skeletal structure
- `pbr_materials` - Physically-based rendering
- `compressed_texture` - DXT/BC formats

### Usage Context
- `gameplay` - Core gameplay elements
- `cinematic` - Cutscene/story content  
- `ui` - Interface elements
- `effects` - Particles, shaders
- `audio_music` - Background music
- `audio_sfx` - Sound effects

## API Schema (JSON)

### Asset Metadata Response
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "entry_id": "object_12345",
  "asset_name": "PlayerCharacter_Mesh",
  "file_type": "Mesh",
  "size_bytes": 2048576,
  "extraction_time": "2025-09-04T10:30:00Z",
  "ai_tags": [
    {
      "tag_name": "character",
      "confidence": 0.92,
      "category": "object_type",
      "ai_model": "clip-vit-b32",
      "processing_time_ms": 1250
    },
    {
      "tag_name": "realistic",
      "confidence": 0.88,
      "category": "art_style", 
      "ai_model": "clip-vit-b32",
      "processing_time_ms": 1250
    }
  ],
  "attributes": {
    "polygon_count": 15420,
    "vertex_count": 8934,
    "has_animations": true,
    "material_count": 3
  },
  "provenance": {
    "source_hash": "abc123...",
    "compliance_profile": "unity_neutral",
    "extraction_session": "sess_789..."
  }
}
```

### Search Request/Response
```json
{
  "query": {
    "tags": ["character", "realistic"],
    "confidence_threshold": 0.85,
    "file_types": ["Mesh", "Texture2D"],
    "attributes": {
      "polygon_count": {"min": 1000, "max": 50000}
    }
  },
  "results": [
    // ... asset objects matching criteria
  ],
  "pagination": {
    "page": 1,
    "limit": 50,
    "total": 1247
  }
}
```

## Migration Strategy

### Phase 1: Schema Creation
- Create tables with migration scripts
- Add foreign key constraints to existing Unity plugin data
- Populate basic asset records from existing extractions

### Phase 2: AI Integration  
- Begin populating `ai_tags` from CLIP processing
- Add consent tracking for enterprise users
- Implement search API with tag filtering

### Phase 3: Enhancement
- Add custom tag categories
- Implement tag confidence tuning
- Add user feedback loops for accuracy improvement

## Performance Considerations

### Indexing Strategy
- Primary indexes on foreign keys and search fields
- Full-text search on tag names using PostgreSQL's text search
- Partial indexes on high-confidence tags (confidence > 0.8)

### Query Optimization
- Pre-computed tag frequency tables for faster faceted search
- Materialized views for common tag combinations
- Connection pooling for high-concurrency access

## Compliance Integration

### Audit Trail
- All AI processing decisions logged with timestamps
- Model versions tracked for reproducibility
- User consent explicitly recorded and retrievable

### Data Retention
- Configurable retention policies per enterprise deployment
- GDPR-compliant data deletion on user request
- Asset anonymization while preserving research value

## Backward Compatibility

### Unity Plugin Integration
- Existing `EntryId` maps directly to `entry_id` field
- Provenance data preserved in JSONB field
- No breaking changes to existing extraction workflow

### API Compatibility
- New AI endpoints are additive to existing API
- Existing search still works without AI tags
- Optional AI features can be disabled per deployment

---

**Status**: Design Complete
**Owner**: PM + Backend Engineer
**Dependencies**: AI Service Architecture
**Next Steps**: Database migration scripts + API endpoint design

**Reviewed By**: [Pending - Security & Compliance]
**Approved For**: Sprint 2 Implementation
