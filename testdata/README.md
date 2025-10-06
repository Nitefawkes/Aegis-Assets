# Test Data Corpus

This directory contains test assets and sample files used for benchmarking, validation, and regression testing of the Aegis-Assets extraction pipeline.

## Directory Structure

```
testdata/
├── unity/          # Unity engine test files and manifest
├── unreal/         # Unreal engine test files (future)
├── godot/          # Godot engine test files (future)
└── golden/         # Golden test outputs for validation
```

## Unity Test Corpus

The Unity test corpus (`testdata/unity/`) contains 6 representative Unity asset bundles covering different scenarios:

- **Mobile games** with texture/audio focus and different compression (LZ4/LZMA)
- **PC games** with mesh-heavy content and large mixed bundles
- **Streaming tests** for memory pressure validation
- **Reference files** for quick smoke tests

See `testdata/unity/manifest.yaml` for detailed specifications.

## Sourcing Test Files

**⚠️ Important:** All test files must be legally obtained and properly attributed.

### Recommended Sources

1. **Unity Sample Projects**
   - Unity Learn tutorials and sample projects
   - Unity Asset Store free assets with permissive licenses
   - Unity Technologies official demo projects

2. **Open Source Games**
   - itch.io games released under CC0 or CC-BY licenses
   - GitHub repositories with Unity projects and permissive licenses
   - Public domain game assets from OpenGameArt.org

3. **Community Contributions**
   - Anonymized samples from community members (with explicit permission)
   - Test files created specifically for Aegis-Assets development

### Attribution Requirements

Each test file must include:
- `source.txt` file documenting origin and license
- Original creator attribution
- License terms (CC0, CC-BY, MIT, etc.)
- Date obtained and any modification notes

## Using the Test Corpus

### Performance Benchmarking

```bash
# Run benchmark suite on Unity corpus
cargo run --bin bench -- --corpus testdata/unity/ --format json

# Memory profiling
cargo run --bin bench -- --corpus testdata/unity/ --profile memory

# Streaming validation  
cargo run --bin bench -- --corpus testdata/unity/ --validate streaming
```

### Golden Test Validation

```bash
# Generate golden outputs
cargo test --test golden_tests -- --generate

# Validate against golden outputs
cargo test --test golden_tests
```

### CI Integration

The test corpus is used in CI for:
- Performance regression detection (>10% throughput drop fails build)
- Memory usage validation (>300MB p95 RSS fails build)
- Format conversion accuracy (golden test mismatches fail build)

## File Size Guidelines

- **Individual files:** 5MB - 500MB each
- **Total corpus size:** Target ~1GB for reasonable CI times
- **Compression ratio:** Files should demonstrate good compression characteristics

## Privacy and Security

- No personal data or sensitive content in test files
- All assets must be appropriate for public distribution
- Regular security scans of test corpus for malware
- Files should not contain Easter eggs or hidden content

## Adding New Test Files

1. Verify licensing and obtain permission
2. Document source in `source.txt`
3. Add entry to appropriate manifest.yaml
4. Test with extraction pipeline
5. Generate golden outputs if needed
6. Update CI configuration if necessary

## Maintenance

- Review test corpus quarterly for relevance
- Update golden outputs when extraction logic changes
- Remove or replace files with licensing issues
- Monitor corpus size to keep CI times reasonable
