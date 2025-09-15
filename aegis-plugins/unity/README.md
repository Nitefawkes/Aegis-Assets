# Unity Plugin for Aegis-Assets

The Unity plugin provides comprehensive support for extracting and converting assets from Unity engine games and applications.

## 🎯 Features

### Supported Formats
- **UnityFS** (.unity3d) - Unity 5.3+ asset bundles
- **Serialized Assets** (.assets, .sharedAssets) - Unity asset files
- **Resources** (.resource, .resS) - Unity resource files

### Asset Types
- **Textures** → PNG conversion with support for:
  - RGBA32, ARGB32, BGRA32
  - RGB24, Alpha8
  - DXT1/DXT5 (with warnings about compression)
  - Multiple Unity texture formats
- **Meshes** → glTF 2.0 conversion
- **Audio Clips** → OGG Vorbis conversion
- **Materials**, **GameObjects**, and other Unity objects

### Compression Support
- **LZ4** - Fast decompression for modern Unity versions
- **LZMA** - High compression ratio for older versions
- **LZ4HC** - High compression LZ4 variant
- **Uncompressed** - Direct extraction

## 🔧 Technical Implementation

### Architecture
```
Unity Plugin
├── formats.rs      - UnityFS & SerializedFile parsers
├── compression.rs  - Multi-algorithm decompression
├── converters.rs   - Asset type conversion (PNG, glTF, OGG)
└── lib.rs         - Main plugin interface
```

### Unity Version Support
- **Unity 3.x-4.x**: Basic serialized file support
- **Unity 5.0-5.2**: UnityRaw format support
- **Unity 5.3+**: Full UnityFS format support
- **Unity 2017+**: Enhanced serialization support
- **Unity 2018-2023**: Modern UnityFS with optimizations

## 🚀 Usage

### Command Line
```bash
# List assets in a Unity file
aegis list sharedassets0.assets --plugin unity

# Extract all assets with conversion
aegis extract level1.unity3d -o ./extracted/ --plugin unity --convert

# Extract specific asset types only
aegis extract game.unity3d --filter "*.png,*.gltf" --plugin unity

# Check compliance before extraction
aegis extract game.unity3d --compliance-check --plugin unity
```

### Python API
```python
from aegis import UnityPlugin

# Open Unity file
plugin = UnityPlugin()
archive = plugin.open("sharedassets0.assets")

# List available assets
for entry in archive.list_entries():
    print(f"{entry.name} ({entry.file_type}) - {entry.size_uncompressed} bytes")

# Extract and convert specific asset
texture_data = archive.read_converted_entry("Texture2D_12345")
with open("texture.png", "wb") as f:
    f.write(texture_data)
```

### Rust API
```rust
use aegis_unity_plugin::{UnityArchive, UnityPluginFactory};
use aegis_core::PluginFactory;

// Create plugin factory
let factory = UnityPluginFactory;

// Check if file is supported
let file_data = std::fs::read("game.unity3d")?;
if factory.can_handle(&file_data) {
    // Open archive
    let archive = UnityArchive::open(Path::new("game.unity3d"))?;
    
    // List entries
    for entry in archive.list_entries()? {
        println!("Asset: {} ({})", entry.name, entry.file_type.unwrap_or_default());
    }
    
    // Extract specific asset
    let asset_data = archive.read_entry(&entry_id)?;
}
```

## ⚖️ Compliance & Legal

### Compliance Profile
- **Publisher**: Unity Technologies
- **Enforcement Level**: Neutral
- **Official Support**: Community-driven
- **Bounty Eligible**: Yes
- **Enterprise Warning**: "Unity games have varying IP policies. Check publisher-specific compliance."

### Legal Considerations
- Unity engine files contain game assets owned by respective publishers
- This plugin extracts assets from **legally owned** game files only
- Extracted assets are for **personal use, research, and preservation**
- **No copyrighted content is redistributed** by this plugin
- Publishers may have specific policies about asset extraction

### Risk Management
- Automatic compliance checking before extraction
- Audit trails for enterprise users
- Publisher-specific warnings when available
- Support for compliance profiles and risk indicators

## 🧪 Testing

### Unit Tests
```bash
# Run Unity plugin tests
cd aegis-plugins/unity
cargo test

# Run specific test suites
cargo test test_unity_format_detection
cargo test test_asset_conversion
cargo test integration_tests
```

### Integration Testing
```bash
# Test with sample Unity files
python examples/unity_extraction.py find ./test_data/
python examples/unity_extraction.py extract test_file.unity3d ./output/
```

### Supported Test Files
- **UnityFS bundles** - Modern Unity asset bundles
- **Legacy assets** - Older Unity serialized files  
- **Compressed bundles** - LZ4/LZMA compressed files
- **Multi-object files** - Files with textures, meshes, audio

## 📊 Performance

### Benchmarks (approximate)
- **Small assets** (< 1MB): ~50-100ms extraction time
- **Medium bundles** (1-50MB): ~200ms-2s extraction time  
- **Large bundles** (50MB+): ~2-10s extraction time
- **Compression overhead**: ~10-30% additional time for LZ4, ~50-100% for LZMA

### Memory Usage
- **Streaming extraction**: Minimal memory footprint
- **Compressed files**: 2-3x compressed size during decompression
- **Asset conversion**: Additional 1-2x memory during PNG/glTF generation

## 🔍 Troubleshooting

### Common Issues

**"Unsupported Unity version"**
- Check if the Unity file is from a very old version (< 3.0)
- Verify file integrity and format

**"Compression not supported"**
- Some exotic compression formats may not be implemented
- Consider using Unity Asset Bundle Extractor (UABE) as a pre-processing step

**"Asset conversion failed"**
- Compressed textures (DXT, BC, ASTC) may need specialized decompression
- Mesh parsing is complex and may fail on certain Unity versions

**"Compliance warning"**
- Check if the game publisher has specific asset extraction policies
- Consider using --compliance-ignore flag (not recommended for distribution)

### Debug Mode
```bash
# Enable verbose logging
RUST_LOG=debug aegis extract game.unity3d --plugin unity -v

# Check plugin detection
aegis detect game.unity3d --all-plugins

# Validate file format
aegis validate game.unity3d --plugin unity
```

## 🛠️ Development

### Building
```bash
cd aegis-plugins/unity
cargo build --release
```

### Adding New Asset Types
1. Add parsing logic to `converters.rs`
2. Update `convert_unity_asset()` function
3. Add Unity class ID mapping
4. Implement conversion to standard format
5. Add tests and documentation

### Contributing
- Follow Aegis-Assets compliance guidelines
- Add comprehensive tests for new features
- Update documentation and examples
- Ensure backward compatibility when possible

## 📝 Changelog

### v0.1.0 (Current)
- ✅ Complete Unity plugin functionality
- ✅ UnityFS format parsing
- ✅ LZ4/LZMA decompression support
- ✅ Texture2D → PNG conversion
- ✅ Basic Mesh → glTF conversion
- ✅ AudioClip → OGG conversion
- ✅ Compliance framework integration
- ✅ Comprehensive test suite

### Roadmap
- **v0.1.1**: Enhanced mesh parsing for more Unity versions
- **v0.1.2**: Compressed texture format support (DXT, BC, ASTC)
- **v0.2.0**: Material and shader extraction
- **v0.2.1**: Unity prefab support
- **v0.3.0**: Unity scene file parsing

---

**Need Help?** 
- 📖 [Full Aegis Documentation](../../docs/)
- 💬 [Community Discord](https://discord.gg/aegis-assets)
- 🐛 [Report Issues](https://github.com/Nitefawkes/Aegis-Assets/issues)
- 🤝 [Contribute](../../CONTRIBUTING.md)
