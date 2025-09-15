# Game Art Tag Taxonomy - Phase 2 Sprint 1

## Taxonomy Overview

Comprehensive taxonomy for AI-powered game asset tagging, designed for CLIP and cloud-based recognition systems. Covers visual, technical, and contextual attributes for game assets.

## Primary Categories

### 1. Object Types (`object_type`)
**Purpose**: What the asset represents in game context  
**AI Model Suitability**: High (visual recognition)  
**Confidence Threshold**: 0.8

| Tag | Description | Examples | CLIP Compatibility |
|-----|-------------|----------|-------------------|
| `character` | Humanoid or creature models | Player models, NPCs, enemies | Excellent |
| `environment` | Static world geometry | Buildings, terrain, rocks | Excellent |
| `prop` | Interactive or decorative objects | Furniture, tools, decorations | Good |
| `weapon` | Combat items | Swords, guns, shields | Excellent |
| `vehicle` | Transportation objects | Cars, ships, aircraft | Excellent |
| `architecture` | Structural elements | Walls, columns, doors | Good |
| `vegetation` | Natural plant life | Trees, grass, flowers | Excellent |
| `ui_element` | Interface components | Buttons, panels, icons | Good |
| `particle_effect` | Visual effects textures | Smoke, fire, magic | Fair |
| `skybox` | Environmental backgrounds | Sky, horizon, atmosphere | Good |

### 2. Art Style (`art_style`)
**Purpose**: Visual aesthetic classification  
**AI Model Suitability**: High (style recognition)  
**Confidence Threshold**: 0.75

| Tag | Description | Visual Indicators | Training Priority |
|-----|-------------|------------------|-------------------|
| `realistic` | Photorealistic rendering | High detail, natural lighting | High |
| `stylized` | Artistic/cartoon style | Simplified forms, bold colors | High |
| `pixel_art` | Retro pixel graphics | Blocky, low resolution | Medium |
| `low_poly` | Minimalist geometry | Angular, flat shading | High |
| `anime` | Japanese animation style | Large eyes, distinctive proportions | Medium |
| `cyberpunk` | Futuristic dystopian | Neon, technology, urban decay | Medium |
| `fantasy` | Medieval/magical theme | Swords, magic, castles | High |
| `sci_fi` | Science fiction | Spaceships, robots, advanced tech | High |
| `horror` | Dark/scary aesthetic | Dark colors, unsettling imagery | Medium |
| `cartoon` | Western animation style | Exaggerated features, bright colors | Medium |

### 3. Technical Attributes (`technical`)
**Purpose**: Asset quality and technical specifications  
**AI Model Suitability**: Medium (requires metadata analysis)  
**Confidence Threshold**: 0.85

| Tag | Description | Detection Method | Automated Source |
|-----|-------------|-----------------|------------------|
| `high_poly` | >10,000 polygons | Mesh analysis | Unity metadata |
| `medium_poly` | 1,000-10,000 polygons | Mesh analysis | Unity metadata |
| `low_poly` | <1,000 polygons | Mesh analysis | Unity metadata |
| `animated` | Contains animation data | Asset structure | Unity serialization |
| `rigged` | Has skeletal structure | Bone detection | Unity components |
| `pbr_material` | Physically-based rendering | Shader analysis | Material properties |
| `texture_atlas` | Combined texture mapping | UV analysis | Texture coordinates |
| `compressed_texture` | DXT/BC format | Format detection | Unity texture format |
| `high_resolution` | >1024x1024 pixels | Image analysis | Texture dimensions |
| `tileable` | Seamless texture | Edge analysis | Pattern detection |

### 4. Color Palette (`color_palette`)
**Purpose**: Dominant color classification  
**AI Model Suitability**: High (color analysis)  
**Confidence Threshold**: 0.9

| Tag | Description | HSV Range | Detection Algorithm |
|-----|-------------|-----------|-------------------|
| `monochrome` | Single color/grayscale | Saturation < 0.1 | Color histogram |
| `warm_colors` | Reds, oranges, yellows | Hue 0-60, 300-360 | Dominant color |
| `cool_colors` | Blues, greens, purples | Hue 120-300 | Dominant color |
| `high_contrast` | Strong light/dark variation | Value range > 0.7 | Histogram analysis |
| `pastel` | Soft, light colors | High value, low saturation | Color space analysis |
| `vibrant` | Bright, saturated colors | Saturation > 0.8 | Color intensity |
| `earth_tones` | Natural browns/greens | Specific hue ranges | Palette matching |
| `neon` | Bright electric colors | High saturation + value | Color space analysis |

### 5. Usage Context (`usage_context`)
**Purpose**: Where/how asset is used in game  
**AI Model Suitability**: Low (requires game knowledge)  
**Confidence Threshold**: 0.6

| Tag | Description | Detection Hints | Confidence Boost |
|-----|-------------|----------------|------------------|
| `gameplay` | Core game mechanics | Interactive objects | Object type correlation |
| `cinematic` | Cutscenes/story | High detail, specific camera angles | Quality indicators |
| `ui` | User interface | 2D elements, text | Flat geometry |
| `background` | Environmental filler | Low detail, repetitive | Distance from camera |
| `hero_asset` | Main character/item | High quality, detailed | Polygon count |
| `modular` | Reusable components | Standard dimensions | Naming patterns |
| `decoration` | Non-interactive props | Static, ornamental | No collision |
| `prototype` | Development placeholder | Low quality, standard materials | Basic textures |

### 6. Asset Quality (`quality`)
**Purpose**: Production readiness assessment  
**AI Model Suitability**: Medium (quality indicators)  
**Confidence Threshold**: 0.8

| Tag | Description | Quality Indicators | Automated Metrics |
|-----|-------------|-------------------|-------------------|
| `production_ready` | Final game quality | High poly, detailed textures | Technical metrics |
| `work_in_progress` | In development | Missing textures, placeholder materials | Incomplete data |
| `prototype` | Early development | Basic geometry, minimal detail | Low complexity |
| `placeholder` | Temporary asset | Standard shapes, default materials | Generic patterns |
| `optimized` | Performance-tuned | Good poly/quality ratio | Efficiency metrics |
| `unoptimized` | Needs optimization | Excessive detail for usage | Performance analysis |

## Special Categories

### 7. Content Rating (`content_rating`)
**Purpose**: Content appropriateness  
**AI Model Suitability**: Medium (content analysis)  
**Confidence Threshold**: 0.9 (high threshold for safety)

| Tag | Description | Detection Criteria | Human Review Required |
|-----|-------------|-------------------|----------------------|
| `family_friendly` | All ages appropriate | No violence, suggestive content | No |
| `teen_content` | Mild violence/themes | Weapons, minor blood | Yes |
| `mature_content` | Adult themes | Gore, suggestive imagery | Yes |
| `violence` | Combat-related | Weapons, blood, destruction | Yes |
| `suggestive` | Adult themes (non-explicit) | Revealing clothing, poses | Yes |

### 8. File Characteristics (`file_meta`)
**Purpose**: Technical file properties  
**AI Model Suitability**: None (metadata only)  
**Confidence Threshold**: 1.0 (deterministic)

| Tag | Description | Source | Processing |
|-----|-------------|--------|-----------|
| `large_file` | >50MB uncompressed | File size | Direct measurement |
| `compressed` | Uses compression | Unity format | Format detection |
| `external_reference` | References other assets | Dependency analysis | Link analysis |
| `bundle_asset` | Part of asset bundle | Container analysis | Bundle structure |

## Tag Relationships

### Hierarchical Relationships
```
object_type/character
├── character/humanoid
├── character/creature
└── character/robot

art_style/stylized
├── stylized/cartoon
├── stylized/anime
└── stylized/low_poly
```

### Conflicting Tags
Tags that should not co-occur (validation rules):
- `high_poly` ↔ `low_poly` (mutually exclusive)
- `monochrome` ↔ `vibrant` (contradictory)
- `realistic` ↔ `pixel_art` (different rendering approaches)
- `production_ready` ↔ `prototype` (different quality levels)

### Complementary Tags
Tags that often appear together:
- `character` + `rigged` + `animated`
- `environment` + `modular` + `tileable`
- `weapon` + `high_poly` + `pbr_material`
- `ui_element` + `high_resolution` + `compressed_texture`

## AI Training Priorities

### High Priority (Train First)
1. **Object Types** - Core functionality, high user value
2. **Art Styles** - Strong visual indicators, good accuracy
3. **Color Palettes** - Algorithmic backup available
4. **Quality Assessment** - Technical metrics available

### Medium Priority (Phase 2)
1. **Technical Attributes** - Hybrid AI + metadata approach
2. **Usage Context** - Requires domain knowledge
3. **Content Rating** - Needs human validation

### Low Priority (Future)
1. **File Characteristics** - Fully algorithmic
2. **Specialized subcategories** - Niche use cases

## Confidence Calibration

### Confidence Ranges
- **0.9-1.0**: High confidence, auto-approve
- **0.8-0.9**: Good confidence, minimal review needed
- **0.7-0.8**: Medium confidence, review recommended
- **0.6-0.7**: Low confidence, human review required
- **<0.6**: Very low confidence, flag for manual tagging

### Confidence Boosting Rules
```python
# Pseudo-code for confidence adjustment
def adjust_confidence(base_confidence, tag, asset_metadata):
    adjusted = base_confidence
    
    # Technical tag boost from metadata
    if tag.category == "technical" and has_metadata(asset_metadata):
        adjusted += 0.1
    
    # Complementary tag boost
    if has_complementary_tags(tag, existing_tags):
        adjusted += 0.05
    
    # Conflicting tag penalty
    if has_conflicting_tags(tag, existing_tags):
        adjusted -= 0.2
    
    # File type consistency boost
    if tag_matches_file_type(tag, asset_metadata.file_type):
        adjusted += 0.05
    
    return min(adjusted, 1.0)
```

## Validation Dataset Requirements

### Balanced Training Set
| Category | Target Count | Priority | Collection Method |
|----------|--------------|----------|-------------------|
| Object Types | 500 per major type | High | Curated game assets |
| Art Styles | 300 per style | High | Style-specific collections |
| Technical | 200 per attribute | Medium | Metadata-rich assets |
| Quality | 150 per level | Medium | Development pipeline assets |

### Annotation Guidelines
- **Multi-label**: Assets can have multiple tags
- **Confidence scoring**: Annotators provide confidence levels
- **Consensus requirement**: 2+ annotators must agree for ground truth
- **Regular review**: Monthly taxonomy updates based on accuracy data

## Integration with AI Pipeline

### Tag Suggestion API
```json
POST /api/v1/assets/{asset_id}/suggest-tags

Response:
{
  "asset_id": "uuid-here",
  "suggested_tags": [
    {
      "tag": "character",
      "category": "object_type", 
      "confidence": 0.92,
      "source_model": "clip-vit-b32",
      "reasoning": "Clear humanoid silhouette with characteristic proportions"
    },
    {
      "tag": "realistic",
      "category": "art_style",
      "confidence": 0.87,
      "source_model": "clip-vit-b32", 
      "reasoning": "High detail textures and natural lighting"
    }
  ],
  "requires_review": ["mature_content"],
  "confidence_score": 0.89
}
```

### Human Review Interface
```json
POST /api/v1/assets/{asset_id}/review-tags

Request:
{
  "reviewed_tags": [
    {
      "tag": "character",
      "approved": true,
      "corrected_confidence": 0.95
    },
    {
      "tag": "mature_content", 
      "approved": false,
      "reason": "No adult themes present"
    }
  ],
  "additional_tags": [
    {
      "tag": "fantasy",
      "category": "art_style",
      "confidence": 0.9,
      "source": "human_reviewer"
    }
  ]
}
```

## Compliance Integration

### Content Filtering
- Automatic flagging of mature content for review
- Enterprise policy enforcement (e.g., no violence tags)
- Audit trail for all tagging decisions
- User consent tracking for AI processing

### Privacy Considerations
- No personally identifiable information in tags
- Content-based tags only (no user behavior data)
- Configurable tag visibility for enterprise deployments
- Retention policies for tag history

---

**Status**: Taxonomy v1.0 Complete  
**Coverage**: 8 categories, 60+ tags  
**AI Compatibility**: Optimized for CLIP + cloud hybrid  
**Next Steps**: Begin training dataset collection  

**Approval Required From**:
- [ ] AI Engineering (training feasibility)
- [ ] Compliance Team (content guidelines) 
- [ ] Product Management (user value prioritization)
- [ ] Community Manager (terminology consistency)

**Implementation Timeline**:
- **Sprint 2**: Core categories (object_type, art_style, color_palette)
- **Sprint 3**: Technical attributes and quality assessment  
- **Sprint 4**: Usage context and content rating with human review
