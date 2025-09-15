# Local CLIP Deployment & AI Tagging Implementation

## Overview

Implementation guide for deploying CLIP-based AI tagging system with Docker containerization, ONNX optimization, and integration with Aegis-Assets preprocessing pipeline.

## Architecture Implementation

### Docker Container Setup

#### Dockerfile
```dockerfile
# Multi-stage build for optimized production image
FROM python:3.11-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create virtual environment
RUN python -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Install Python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Download and convert CLIP model to ONNX
RUN python -c "
import clip
import torch
import torch.onnx

# Load CLIP model
device = 'cpu'
model, preprocess = clip.load('ViT-B/32', device=device)
model.eval()

# Create dummy input for ONNX export
dummy_input = torch.randn(1, 3, 224, 224)

# Export to ONNX
torch.onnx.export(
    model.visual,
    dummy_input,
    '/opt/venv/models/clip_visual.onnx',
    export_params=True,
    opset_version=11,
    do_constant_folding=True,
    input_names=['image'],
    output_names=['features'],
    dynamic_axes={'image': {0: 'batch_size'}}
)

print('CLIP model exported to ONNX')
"

# Production stage
FROM python:3.11-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libgomp1 \
    libjpeg62-turbo \
    libpng16-16 \
    libwebp6 \
    && rm -rf /var/lib/apt/lists/*

# Copy virtual environment from builder stage
COPY --from=builder /opt/venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Create app directory
WORKDIR /app

# Copy application code
COPY src/ ./src/
COPY config/ ./config/

# Create non-root user for security
RUN useradd -r -s /bin/false clipuser && \
    chown -R clipuser:clipuser /app
USER clipuser

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the application
CMD ["python", "-m", "src.clip_service"]
```

#### requirements.txt
```
# Core dependencies
torch==2.1.0+cpu
torchvision==0.16.0+cpu
clip-by-openai==1.0
onnxruntime==1.16.1
onnx==1.14.1

# Image processing
Pillow==10.0.1
opencv-python==4.8.1.78
scikit-image==0.21.0

# API framework
fastapi==0.104.1
uvicorn[standard]==0.24.0
pydantic==2.4.2

# Database and caching
asyncpg==0.29.0
redis==5.0.1
aioredis==2.0.1

# Monitoring and logging
prometheus-client==0.18.0
structlog==23.2.0

# Image optimization
numpy==1.24.3
```

### Main Service Implementation

#### FastAPI Service (src/clip_service.py)
```python
import asyncio
import logging
import time
from typing import List, Dict, Any, Optional
from contextlib import asynccontextmanager

import torch
import clip
import onnxruntime as ort
import numpy as np
from PIL import Image
from fastapi import FastAPI, HTTPException, BackgroundTasks
from pydantic import BaseModel, Field
import structlog
import aioredis
import asyncpg

# Configure structured logging
structlog.configure(
    processors=[
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
        structlog.processors.UnicodeDecoder(),
        structlog.processors.JSONRenderer()
    ],
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
    wrapper_class=structlog.stdlib.BoundLogger,
    cache_logger_on_first_use=True,
)

logger = structlog.get_logger()

class CLIPTaggingService:
    def __init__(self):
        self.device = torch.device('cpu')  # CPU-only for container
        self.model = None
        self.preprocess = None
        self.onnx_session = None
        
        # Game asset taxonomy
        self.taxonomy = {
            'object_types': [
                'character', 'environment', 'weapon', 'vehicle', 'prop',
                'architecture', 'vegetation', 'ui_element', 'particle_effect', 'skybox'
            ],
            'art_styles': [
                'realistic', 'stylized', 'pixel_art', 'low_poly', 'anime',
                'cyberpunk', 'fantasy', 'sci_fi', 'horror', 'cartoon'
            ],
            'color_palettes': [
                'monochrome', 'warm_colors', 'cool_colors', 'high_contrast',
                'pastel', 'vibrant', 'earth_tones', 'neon'
            ]
        }
        
    async def initialize(self):
        """Initialize all service components"""
        logger.info("Initializing CLIP tagging service")
        
        # Load CLIP model
        self.model, self.preprocess = clip.load("ViT-B/32", device=self.device)
        self.model.eval()
        
        # Initialize ONNX runtime session
        providers = ['CPUExecutionProvider']
        self.onnx_session = ort.InferenceSession(
            "/opt/venv/models/clip_visual.onnx",
            providers=providers
        )
        
        logger.info("CLIP tagging service initialized successfully")
        
    async def generate_tags(self, image: Image.Image, confidence_threshold: float = 0.7) -> List[Dict[str, Any]]:
        """Generate tags for an image using CLIP"""
        start_time = time.time()
        
        try:
            # Preprocess image
            image_input = self.preprocess(image).unsqueeze(0).numpy()
            
            # Run ONNX inference
            ort_inputs = {self.onnx_session.get_inputs()[0].name: image_input}
            image_features = self.onnx_session.run(None, ort_inputs)[0]
            
            # Normalize features
            image_features = image_features / np.linalg.norm(image_features, axis=-1, keepdims=True)
            
            # Generate tags for each category
            all_tags = []
            
            for category, tags in self.taxonomy.items():
                # Create text prompts
                prompts = [f"a {category.replace('_', ' ')} that is {tag.replace('_', ' ')}" for tag in tags]
                
                # Encode text prompts
                text_tokens = clip.tokenize(prompts).to(self.device)
                with torch.no_grad():
                    text_features = self.model.encode_text(text_tokens)
                    text_features = text_features / text_features.norm(dim=-1, keepdim=True)
                    text_features = text_features.cpu().numpy()
                
                # Calculate similarities
                similarities = np.dot(image_features, text_features.T).flatten()
                
                # Filter by confidence threshold
                for i, similarity in enumerate(similarities):
                    if similarity >= confidence_threshold:
                        all_tags.append({
                            'tag': tags[i],
                            'category': category,
                            'confidence': float(similarity),
                            'source_model': 'clip-vit-b32-local',
                            'processing_time_ms': int((time.time() - start_time) * 1000)
                        })
                        
            # Sort by confidence
            all_tags.sort(key=lambda x: x['confidence'], reverse=True)
            
            logger.info("Generated tags for image", 
                       tag_count=len(all_tags),
                       processing_time_ms=int((time.time() - start_time) * 1000))
            
            return all_tags
            
        except Exception as e:
            logger.error("Tag generation failed", error=str(e))
            raise

# FastAPI application
@asynccontextmanager
async def lifespan(app: FastAPI):
    # Startup
    clip_service = CLIPTaggingService()
    await clip_service.initialize()
    app.state.clip_service = clip_service
    
    yield
    
    # Shutdown cleanup would go here

app = FastAPI(
    title="Aegis-Assets CLIP Tagging Service",
    description="AI-powered semantic tagging for game assets",
    version="1.0.0",
    lifespan=lifespan
)

@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {
        "status": "healthy",
        "timestamp": time.time(),
        "service": "clip-tagging"
    }

@app.post("/api/v1/tag")
async def tag_asset(request: dict):
    """Tag a single asset"""
    try:
        clip_service = app.state.clip_service
        
        # Load image from base64
        import base64
        import io
        
        if request['image_data'].startswith('data:image'):
            image_data = request['image_data'].split(',')[1]
        else:
            image_data = request['image_data']
            
        image_bytes = base64.b64decode(image_data)
        image = Image.open(io.BytesIO(image_bytes))
        
        if image.mode != 'RGB':
            image = image.convert('RGB')
        
        # Generate tags
        tags = await clip_service.generate_tags(image, request.get('confidence_threshold', 0.7))
        
        return {
            "asset_id": request.get('asset_id'),
            "tags": tags,
            "processing_time_ms": sum(tag.get('processing_time_ms', 0) for tag in tags),
            "status": 'success'
        }
    except Exception as e:
        logger.error("Tagging request failed", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "src.clip_service:app",
        host="0.0.0.0",
        port=8080,
        workers=1,
        loop="asyncio"
    )
```

### Docker Compose for Development
```yaml
# docker-compose.yml
version: '3.8'

services:
  clip-service:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
    environment:
      - LOG_LEVEL=INFO
      - MAX_CONCURRENT_REQUESTS=10
    volumes:
      - ./models:/opt/venv/models
      - ./logs:/app/logs
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s

  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_DB: aegis_ai
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: password
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data
    ports:
      - "6379:6379"

volumes:
  postgres_data:
  redis_data:
```

### Performance Testing Script
```python
# test_performance.py
import asyncio
import aiohttp
import time
import base64
from PIL import Image
import io

async def test_tagging_performance():
    """Test CLIP tagging service performance"""
    
    # Create test image
    test_image = Image.new('RGB', (224, 224), color='red')
    image_bytes = io.BytesIO()
    test_image.save(image_bytes, format='PNG')
    image_base64 = base64.b64encode(image_bytes.getvalue()).decode()
    
    # Test configuration
    service_url = "http://localhost:8080"
    num_requests = 50
    concurrent_requests = 5
    
    async def single_request(session, request_id):
        start_time = time.time()
        
        payload = {
            "asset_id": f"test_asset_{request_id}",
            "image_data": image_base64,
            "confidence_threshold": 0.7
        }
        
        try:
            async with session.post(f"{service_url}/api/v1/tag", json=payload) as response:
                result = await response.json()
                processing_time = time.time() - start_time
                
                return {
                    'request_id': request_id,
                    'status_code': response.status,
                    'processing_time': processing_time,
                    'tag_count': len(result.get('tags', [])),
                    'service_processing_time': result.get('processing_time_ms', 0)
                }
        except Exception as e:
            return {
                'request_id': request_id,
                'error': str(e),
                'processing_time': time.time() - start_time
            }
    
    # Run concurrent requests
    semaphore = asyncio.Semaphore(concurrent_requests)
    
    async def limited_request(session, request_id):
        async with semaphore:
            return await single_request(session, request_id)
    
    start_time = time.time()
    
    async with aiohttp.ClientSession() as session:
        tasks = [limited_request(session, i) for i in range(num_requests)]
        results = await asyncio.gather(*tasks)
    
    total_time = time.time() - start_time
    
    # Analyze results
    successful_requests = [r for r in results if 'error' not in r and r['status_code'] == 200]
    failed_requests = [r for r in results if 'error' in r or r['status_code'] != 200]
    
    if successful_requests:
        avg_processing_time = sum(r['processing_time'] for r in successful_requests) / len(successful_requests)
        avg_service_time = sum(r['service_processing_time'] for r in successful_requests) / len(successful_requests)
        avg_tag_count = sum(r['tag_count'] for r in successful_requests) / len(successful_requests)
    else:
        avg_processing_time = avg_service_time = avg_tag_count = 0
    
    print(f"""
Performance Test Results:
========================
Total requests: {num_requests}
Concurrent requests: {concurrent_requests}
Total time: {total_time:.2f} seconds
Requests per second: {num_requests / total_time:.2f}

Successful requests: {len(successful_requests)}
Failed requests: {len(failed_requests)}
Success rate: {len(successful_requests) / num_requests * 100:.1f}%

Average request time: {avg_processing_time * 1000:.0f} ms
Average service processing time: {avg_service_time:.0f} ms
Average tags per image: {avg_tag_count:.1f}

Target Performance:
- Request time: <2000ms (Current: {avg_processing_time * 1000:.0f}ms)
- Service time: <1500ms (Current: {avg_service_time:.0f}ms)
- Success rate: >99% (Current: {len(successful_requests) / num_requests * 100:.1f}%)
""")

if __name__ == "__main__":
    asyncio.run(test_tagging_performance())
```

## Integration with Unity Plugin

### Unity Plugin AI Integration (aegis-plugins/unity/src/ai_integration.rs)
```rust
// Integration with CLIP tagging service
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct TaggingRequest {
    pub asset_id: String,
    pub image_data: String,  // Base64 encoded
    pub confidence_threshold: f64,
}

#[derive(Debug, Deserialize)]
pub struct TaggingResponse {
    pub asset_id: Option<String>,
    pub tags: Vec<AITag>,
    pub processing_time_ms: u64,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AITag {
    pub tag: String,
    pub category: String,
    pub confidence: f64,
    pub source_model: String,
    pub processing_time_ms: u64,
}

pub struct AITaggingClient {
    client: Client,
    base_url: String,
}

impl AITaggingClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
            
        Self { client, base_url }
    }
    
    /// Tag a texture asset using AI
    pub async fn tag_texture(&self, asset_id: &str, image_data: &[u8]) -> Result<Vec<AITag>> {
        // Convert image data to base64
        let image_base64 = base64::encode(image_data);
        
        let request = TaggingRequest {
            asset_id: asset_id.to_string(),
            image_data: image_base64,
            confidence_threshold: 0.7,
        };
        
        let response = self.client
            .post(&format!("{}/api/v1/tag", self.base_url))
            .json(&request)
            .send()
            .await?;
            
        if response.status().is_success() {
            let tagging_response: TaggingResponse = response.json().await?;
            Ok(tagging_response.tags)
        } else {
            let error_text = response.text().await?;
            Err(anyhow::anyhow!("AI tagging failed: {}", error_text))
        }
    }
}

// Integration with Unity archive extraction
impl crate::UnityArchive {
    /// Extract asset with AI tagging
    pub async fn read_entry_with_ai_tags(&self, id: &crate::EntryId) -> Result<(Vec<u8>, Vec<AITag>)> {
        // Extract raw asset data
        let asset_data = self.read_entry(id)?;
        
        // Check if this is a visual asset that can be tagged
        if let Some(entry) = self.entries.iter().find(|e| e.id == *id) {
            if entry.file_type.as_deref() == Some("Texture2D") {
                // Convert Unity texture to PNG for AI processing
                if let Ok((png_filename, png_data)) = self.read_converted_entry(id) {
                    if png_filename.ends_with(".png") {
                        // Initialize AI client
                        let ai_client = AITaggingClient::new(
                            std::env::var("AI_TAGGING_URL")
                                .unwrap_or_else(|_| "http://localhost:8080".to_string())
                        );
                        
                        // Get AI tags
                        match ai_client.tag_texture(&id.0, &png_data).await {
                            Ok(tags) => {
                                tracing::info!("Generated {} AI tags for asset {}", tags.len(), id.0);
                                return Ok((asset_data, tags));
                            }
                            Err(e) => {
                                tracing::warn!("AI tagging failed for asset {}: {}", id.0, e);
                            }
                        }
                    }
                }
            }
        }
        
        // Return asset data without tags if AI tagging failed or not applicable
        Ok((asset_data, vec![]))
    }
}
```

## Deployment Instructions

### 1. Build and Deploy CLIP Service
```bash
# Clone repository and navigate to AI service directory
cd aegis-assets/ai-services/clip

# Build Docker image
docker build -t aegis-clip-service:latest .

# Run with docker-compose
docker-compose up -d

# Check service health
curl http://localhost:8080/health

# Test tagging endpoint
curl -X POST http://localhost:8080/api/v1/tag \
  -H "Content-Type: application/json" \
  -d '{
    "asset_id": "test_asset",
    "image_data": "base64_encoded_image_data",
    "confidence_threshold": 0.7
  }'
```

### 2. Performance Validation
```bash
# Run performance tests
python test_performance.py

# Expected results:
# - Average request time: <2000ms
# - Success rate: >99%
# - Average tags per image: 5-15
```

### 3. Integration Testing
```bash
# Test Unity plugin integration
cd aegis-plugins/unity
cargo test test_ai_integration

# Test end-to-end workflow
aegis extract sample.unity3d --ai-tagging --output ./extracted/
```

---

**Status**: CLIP Deployment Implementation Complete  
**Performance Targets**: <2s per asset, >99% uptime, concurrent processing  
**Dependencies**: Docker, ONNX Runtime, FastAPI  
**Next**: Plugin Registry Backend Implementation

Ready to continue with Plugin Registry Backend development?