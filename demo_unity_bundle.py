#!/usr/bin/env python3
"""
Demo Unity Bundle Creator
Creates a realistic Unity asset bundle for testing Aegis-Assets functionality
"""

import struct
import os
from pathlib import Path

def create_minimal_unityfs_bundle(filename: str):
    """Create a minimal but structurally valid UnityFS bundle for testing"""
    
    with open(filename, 'wb') as f:
        # UnityFS Signature (8 bytes)
        f.write(b'UnityFS\x00')
        
        # Format version (4 bytes, big endian) - Unity 2022.3
        f.write(struct.pack('>I', 7))
        
        # Unity version string (null-terminated)
        unity_version = b'2022.3.15f1\x00'
        f.write(unity_version)
        
        # Unity revision string (null-terminated) 
        unity_revision = b'b58023a2fb5c\x00'
        f.write(unity_revision)
        
        # Bundle size (8 bytes, big endian)
        bundle_size = 256  # Small but reasonable size
        f.write(struct.pack('>Q', bundle_size))
        
        # Compressed blocks info size (4 bytes, big endian)
        f.write(struct.pack('>I', 48))
        
        # Uncompressed blocks info size (4 bytes, big endian)
        f.write(struct.pack('>I', 48))
        
        # Flags (4 bytes, big endian) - 0x00 = uncompressed
        f.write(struct.pack('>I', 0x00))
        
        # Block info (48 bytes total for 1 block)
        # Uncompressed size (4 bytes)
        f.write(struct.pack('>I', 128))
        # Compressed size (4 bytes)  
        f.write(struct.pack('>I', 128))
        # Flags (2 bytes)
        f.write(struct.pack('>H', 0))
        
        # Directory info (1 entry)
        # Offset (8 bytes)
        f.write(struct.pack('>Q', 128))
        # Size (8 bytes) 
        f.write(struct.pack('>Q', 64))
        # Flags (4 bytes)
        f.write(struct.pack('>I', 0))
        # Name: "SharedAssets0.assets" + null terminator
        name = b'SharedAssets0.assets\x00'
        f.write(name)
        
        # Pad to reach our declared bundle size
        current_pos = f.tell()
        remaining = bundle_size - current_pos
        if remaining > 0:
            f.write(b'\x00' * remaining)

def create_texture_demo_bundle(filename: str):
    """Create a bundle that simulates containing texture assets"""
    
    with open(filename, 'wb') as f:
        # UnityFS2 Signature (8 bytes) - newer format
        f.write(b'UnityFS2')
        
        # Format version (4 bytes, big endian) - Unity 2023.1
        f.write(struct.pack('>I', 8))
        
        # Unity version string
        f.write(b'2023.1.0f1\x00')
        
        # Unity revision string
        f.write(b'c1b978022a1a\x00')
        
        # Bundle size
        bundle_size = 512
        f.write(struct.pack('>Q', bundle_size))
        
        # Block info sizes
        f.write(struct.pack('>I', 64))  # compressed
        f.write(struct.pack('>I', 64))  # uncompressed
        
        # Flags - uncompressed for demo
        f.write(struct.pack('>I', 0x00))
        
        # Create 2 blocks - one for textures, one for materials
        # Block 1: Texture data
        f.write(struct.pack('>I', 256))  # uncompressed size
        f.write(struct.pack('>I', 256))  # compressed size (no compression for demo)
        f.write(struct.pack('>H', 0))    # flags: uncompressed
        
        # Block 2: Material data  
        f.write(struct.pack('>I', 128))  # uncompressed size
        f.write(struct.pack('>I', 128))  # compressed size (no compression)
        f.write(struct.pack('>H', 0))    # flags: uncompressed
        
        # Directory entries
        # Entry 1: MainTexture.png
        f.write(struct.pack('>Q', 200))  # offset
        f.write(struct.pack('>Q', 256))  # size
        f.write(struct.pack('>I', 4))    # flags: texture
        f.write(b'MainTexture\x00')
        
        # Entry 2: Material.mat
        f.write(struct.pack('>Q', 456))  # offset  
        f.write(struct.pack('>Q', 128))  # size
        f.write(struct.pack('>I', 8))    # flags: material
        f.write(b'StandardMaterial\x00')
        
        # Pad to target size
        current_pos = f.tell()
        remaining = bundle_size - current_pos
        if remaining > 0:
            f.write(b'\x00' * remaining)

def main():
    """Create demo Unity bundles and show usage"""
    
    print("🎮 Creating Unity Demo Bundles for Aegis-Assets Testing")
    print("=" * 60)
    
    # Create demo bundles
    create_minimal_unityfs_bundle("demo_basic.unity3d")
    create_texture_demo_bundle("demo_textures.unity3d")
    
    print("✅ Created demo_basic.unity3d (minimal UnityFS bundle)")
    print("✅ Created demo_textures.unity3d (texture-focused bundle)")
    print()
    
    print("🚀 Test Commands:")
    print("# List assets in basic bundle:")
    print("./target/release/aegis list demo_basic.unity3d --details")
    print()
    print("# List assets in texture bundle:")  
    print("./target/release/aegis list demo_textures.unity3d --details")
    print()
    print("# Extract assets with conversion:")
    print("./target/release/aegis extract demo_textures.unity3d -o extracted_demo --convert")
    print()
    print("# Check compliance:")
    print("./target/release/aegis compliance demo_textures.unity3d")
    print()
    print("# Show available plugins:")
    print("./target/release/aegis plugins")

if __name__ == "__main__":
    main()
