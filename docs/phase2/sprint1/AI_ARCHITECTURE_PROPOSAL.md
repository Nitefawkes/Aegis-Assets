# AI Architecture Proposal - Hybrid CLIP + Cloud Fallback

## Architecture Decision Record (ADR)

**Status**: Proposed  
**Date**: 2025-09-04  
**Owner**: AI Engineer  
**Dependencies**: AI Service Comparison Matrix, Metadata Schema v1

## Context

Based on the AI service comparison matrix, we need an architecture that balances:
- **Performance**: <2s processing per asset
- **Accuracy**: >85% top-3 precision on game assets
- **Cost**: <$0.08 per asset processed
- **Privacy**: Enterprise-grade data handling with user controls

## Decision

Implement a **Hybrid Architecture** with local CLIP as primary processor and AWS Rekognition as intelligent fallback for low-confidence cases.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI Tagging Service                           │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────┐    ┌─────────────────────────────────┐ │
│  │   Request Router    │    │      Consent Manager           │ │
│  │   - Confidence      │    │   - User preferences          │ │
│  │   - Privacy rules   │    │   - Enterprise policies       │ │
│  │   - Load balancing  │    │   - Audit logging             │ │
│  └─────────────────────┘    └─────────────────────────────────┘ │
│             │                              │                   │
│  ┌─────────────────────┐    ┌─────────────────────────────────┐ │
│  │   Local CLIP        │    │     Cloud Fallback             │ │
│  │   - Primary (80%)   │    │   - AWS Rekognition (20%)     │ │
│  │   - Docker container│    │   - Custom labels             │ │
│  │   - ONNX optimized  │    │   - High confidence boost     │ │
│  │   - GPU accelerated │    │   - Rate limited              │ │
│  └─────────────────────┘    └─────────────────────────────────┘ │
│             │                              │                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                Tag Aggregation Engine                       │ │
│  │  - Confidence weighting  - Duplicate removal               │ │
│  │  - Quality scoring      - Taxonomy mapping                │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Component Specifications

### 1. Local CLIP Container

#### Docker Configuration
```dockerfile
FROM nvidia/cuda:11.8-runtime-ubuntu22.04

RUN apt-get update && apt-get install -y \
    python3.10 python3-pip \
    libgl1-mesa-glx libglib2.0-0

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Download and optimize CLIP model
RUN python3 -c "import clip; clip.load('ViT-B/32')"

COPY src/ /app/src/
WORKDIR /app

# Health check endpoint
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s \
  CMD python3 -c "import requests; requests.get('http://localhost:8080/health')"

EXPOSE 8080
CMD ["python3", "-m", "src.clip_service"]
```

#### ONNX Optimization
- Convert PyTorch CLIP to ONNX format for 40% inference speedup
- Use NVIDIA TensorRT for additional GPU optimization
- Batch processing for multiple assets (max batch size: 32)

#### Performance Targets
- **Latency**: 0.8s average (0.5s best case, 1.5s worst case)
- **Throughput**: 50 assets/minute on RTX 3080
- **Memory**: <8GB GPU memory usage
- **CPU**: <4 cores during processing

### 2. Cloud Fallback Service

#### AWS Rekognition Integration
```python
import boto3
from botocore.config import Config

# Configured for reliability and cost control
rekognition = boto3.client(
    'rekognition',
    config=Config(
        retries={'max_attempts': 3, 'mode': 'adaptive'},
        max_pool_connections=10
    )
)

async def process_with_rekognition(image_bytes, custom_labels=True):
    response = await rekognition.detect_custom_labels(
        Image={'Bytes': image_bytes},
        ProjectVersionArn='arn:aws:rekognition:us-east-1:...:project/game-assets/version/1.0',
        MaxResults=10,
        MinConfidence=70
    )
    return parse_rekognition_response(response)
```

#### Trigger Conditions
- Local CLIP confidence <0.75 for any top-3 tag
- Asset type not well-handled by CLIP (e.g., technical diagrams)
- User explicitly requests cloud processing
- Batch processing mode for high-accuracy requirements

### 3. Request Router Logic

#### Decision Tree
```python
async def route_request(asset_data, user_preferences, enterprise_policy):
    # Check consent and privacy policies
    if not check_consent(asset_data.id, user_preferences):
        return await process_local_only(asset_data)
    
    # Try local processing first
    local_result = await process_local_clip(asset_data)
    
    # Evaluate if fallback needed
    if should_fallback(local_result, user_preferences):
        cloud_result = await process_cloud_fallback(asset_data)
        return merge_results(local_result, cloud_result)
    
    return local_result

def should_fallback(local_result, preferences):
    max_confidence = max(tag.confidence for tag in local_result.tags)
    
    return (
        max_confidence < preferences.min_confidence_threshold or
        len(local_result.tags) < preferences.min_tag_count or
        preferences.accuracy_mode == "high"
    )
```

### 4. Tag Aggregation Engine

#### Confidence Weighting
- Local CLIP: Base confidence score
- AWS Rekognition: +0.1 confidence boost for matching tags
- Agreement bonus: +0.05 when both services agree
- Conflict resolution: Higher-confidence source wins

#### Quality Scoring
```python
def calculate_quality_score(tags, processing_time, model_versions):
    base_score = sum(tag.confidence for tag in tags) / len(tags)
    
    # Penalty for slow processing (indicates model uncertainty)
    time_penalty = min(processing_time / 2000, 0.1)  # Max 0.1 penalty at 2s
    
    # Bonus for model agreement
    agreement_bonus = count_model_agreements(tags) * 0.02
    
    # Penalty for very recent model versions (less tested)
    stability_bonus = 0.05 if all(is_stable_version(v) for v in model_versions) else 0
    
    return min(base_score - time_penalty + agreement_bonus + stability_bonus, 1.0)
```

## Deployment Architecture

### Local Development
```yaml
# docker-compose.dev.yml
version: '3.8'
services:
  clip-service:
    build: ./ai-services/clip
    ports:
      - "8080:8080"
    environment:
      - CUDA_VISIBLE_DEVICES=0
      - MODEL_CACHE=/models
    volumes:
      - ./models:/models
      - /tmp:/tmp
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
```

### Production Enterprise
```yaml
# k8s-deployment.yml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ai-tagging-service
spec:
  replicas: 2
  template:
    spec:
      containers:
      - name: clip-service
        image: aegis/ai-tagging:v1.0
        resources:
          limits:
            nvidia.com/gpu: 1
            memory: "16Gi"
          requests:
            nvidia.com/gpu: 1
            memory: "8Gi"
        env:
        - name: AWS_REGION
          value: "us-east-1"
        - name: REKOGNITION_PROJECT_ARN
          valueFrom:
            secretKeyRef:
              name: aws-credentials
              key: project-arn
```

## Privacy and Compliance

### Consent Management
- **Explicit Opt-in**: Users must explicitly consent to cloud processing
- **Granular Controls**: Separate consent for tagging vs. model training
- **Enterprise Policies**: Admin can disable cloud processing globally
- **Audit Logging**: All consent decisions logged with timestamps

### Data Handling
- **Local Processing**: Asset data never leaves local environment by default
- **Cloud Processing**: Only when explicitly consented, with encryption in transit
- **Data Retention**: Cloud-processed data deleted within 24 hours
- **Model Training**: Explicit separate consent required, disabled by default

### Compliance Integration
```python
@audit_log
async def process_asset_with_consent(asset_id, user_id, processing_options):
    consent = await get_user_consent(user_id, asset_id)
    
    if not consent.allows_ai_processing:
        raise ConsentError("User has not consented to AI processing")
    
    if processing_options.use_cloud and not consent.allows_cloud_processing:
        processing_options.use_cloud = False
        logger.warning(f"Cloud processing disabled for user {user_id}")
    
    result = await process_asset(asset_id, processing_options)
    
    # Log processing decision
    await log_processing_decision(
        asset_id=asset_id,
        user_id=user_id,
        models_used=result.models_used,
        consent_version=consent.version,
        processing_time=result.processing_time
    )
    
    return result
```

## API Design

### Tagging Endpoint
```
POST /api/v1/assets/{asset_id}/tags
Content-Type: application/json
Authorization: Bearer <token>

{
  "processing_options": {
    "use_cloud_fallback": true,
    "min_confidence": 0.8,
    "max_processing_time": 3000,
    "accuracy_mode": "balanced"  // "fast" | "balanced" | "accurate"
  },
  "consent": {
    "version": "1.0",
    "allows_cloud_processing": true,
    "allows_model_training": false
  }
}
```

### Response Format
```json
{
  "asset_id": "550e8400-e29b-41d4-a716-446655440000",
  "tags": [
    {
      "name": "character",
      "confidence": 0.92,
      "category": "object_type",
      "source_model": "clip-vit-b32-local",
      "processing_time_ms": 890
    },
    {
      "name": "realistic",
      "confidence": 0.88,
      "category": "art_style",
      "source_model": "clip-vit-b32-local",
      "processing_time_ms": 890
    }
  ],
  "quality_score": 0.91,
  "total_processing_time": 1250,
  "models_used": ["clip-vit-b32-local"],
  "consent_version": "1.0",
  "processed_at": "2025-09-04T10:30:00Z"
}
```

## Implementation Timeline

### Sprint 1-2 (Weeks 1-4): Foundation
- [ ] Local CLIP Docker container with basic API
- [ ] Metadata schema implementation in database
- [ ] Consent management system design
- [ ] Basic tag taxonomy implementation

### Sprint 3 (Weeks 5-6): Core Integration
- [ ] ONNX optimization for CLIP model
- [ ] AWS Rekognition integration with rate limiting
- [ ] Request routing logic implementation
- [ ] Tag aggregation and quality scoring

### Sprint 4 (Weeks 7-8): Polish
- [ ] Enterprise deployment package
- [ ] Monitoring and alerting setup
- [ ] Performance optimization based on benchmarks
- [ ] Security review and penetration testing

## Success Criteria

### Performance
- [ ] Average processing time <2 seconds per asset
- [ ] 95th percentile processing time <3 seconds per asset
- [ ] Support for 50+ concurrent tagging requests
- [ ] GPU memory usage <8GB under normal load

### Accuracy
- [ ] Top-3 tag accuracy >85% on validation set
- [ ] Quality score >0.8 for 90% of processed assets
- [ ] <5% false positive rate on high-confidence tags
- [ ] Agreement rate >75% between local and cloud when both used

### Compliance
- [ ] 100% of processing decisions logged in audit trail
- [ ] Consent checked and honored for every request
- [ ] Enterprise privacy policies enforced
- [ ] Data retention policies implemented and tested

### Cost
- [ ] Average cost <$0.08 per asset processed
- [ ] Cloud usage <25% of total processing volume
- [ ] Monthly infrastructure cost <$800 at 10K assets/month

---

**Status**: Ready for Implementation  
**Next Phase**: Sprint 2 - Begin Docker container development  
**Risk Level**: Medium (GPU dependencies, cloud integration complexity)

**Implementation Order**:
1. Local CLIP container (Week 1-2)
2. Basic API and routing (Week 3)
3. Cloud integration (Week 4) 
4. Aggregation and quality scoring (Week 5-6)
