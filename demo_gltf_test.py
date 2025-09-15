#!/usr/bin/env python3
"""
Demo script to create a simple mesh and export it to glTF format
This demonstrates the glTF export functionality of Aegis-Assets
"""

import json
import struct
from pathlib import Path

def create_cube_mesh_gltf():
    """Create a simple cube mesh in glTF format"""

    # Cube vertices (positions)
    positions = [
        -0.5, -0.5,  0.5,  # 0: front-bottom-left
         0.5, -0.5,  0.5,  # 1: front-bottom-right
         0.5,  0.5,  0.5,  # 2: front-top-right
        -0.5,  0.5,  0.5,  # 3: front-top-left
        -0.5, -0.5, -0.5,  # 4: back-bottom-left
         0.5, -0.5, -0.5,  # 5: back-bottom-right
         0.5,  0.5, -0.5,  # 6: back-top-right
        -0.5,  0.5, -0.5,  # 7: back-top-left
    ]

    # Cube normals
    normals = [
        0.0, 0.0, 1.0,   # front face
        0.0, 0.0, -1.0,  # back face
        -1.0, 0.0, 0.0,  # left face
        1.0, 0.0, 0.0,   # right face
        0.0, 1.0, 0.0,   # top face
        0.0, -1.0, 0.0,  # bottom face
    ]

    # Cube UV coordinates
    uvs = [
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0,  # front/back
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0,  # left/right
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0,  # top/bottom
    ]

    # Cube indices (triangles)
    indices = [
        # Front face
        0, 1, 2,  0, 2, 3,
        # Back face
        5, 4, 7,  5, 7, 6,
        # Left face
        4, 0, 3,  4, 3, 7,
        # Right face
        1, 5, 6,  1, 6, 2,
        # Top face
        3, 2, 6,  3, 6, 7,
        # Bottom face
        4, 5, 1,  4, 1, 0,
    ]

    # Create binary data
    binary_data = bytearray()

    # Write positions (FLOAT, VEC3)
    for pos in positions:
        binary_data.extend(struct.pack('<f', pos))

    # Write normals (FLOAT, VEC3)
    for normal in normals:
        binary_data.extend(struct.pack('<f', normal))

    # Write UVs (FLOAT, VEC2)
    for uv in uvs:
        binary_data.extend(struct.pack('<f', uv))

    # Write indices (UNSIGNED_SHORT, SCALAR)
    for index in indices:
        binary_data.extend(struct.pack('<H', index))

    # Create glTF JSON structure
    gltf = {
        "asset": {
            "version": "2.0",
            "generator": "Aegis-Assets glTF Demo"
        },
        "scene": 0,
        "scenes": [{
            "name": "Scene",
            "nodes": [0]
        }],
        "nodes": [{
            "name": "Cube",
            "mesh": 0
        }],
        "meshes": [{
            "name": "Cube",
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1,
                    "TEXCOORD_0": 2
                },
                "indices": 3,
                "mode": 4  # TRIANGLES
            }]
        }],
        "bufferViews": [{
            "buffer": 0,
            "byteOffset": 0,
            "byteLength": len(binary_data),
            "target": 34962  # ARRAY_BUFFER
        }],
        "buffers": [{
            "byteLength": len(binary_data),
            "uri": "cube.bin"
        }],
        "accessors": [
            # POSITION
            {
                "bufferView": 0,
                "byteOffset": 0,
                "componentType": 5126,  # FLOAT
                "count": 8,
                "type": "VEC3",
                "min": [-0.5, -0.5, -0.5],
                "max": [0.5, 0.5, 0.5]
            },
            # NORMAL
            {
                "bufferView": 0,
                "byteOffset": 8 * 3 * 4,  # 8 vertices * 3 floats * 4 bytes
                "componentType": 5126,  # FLOAT
                "count": 8,
                "type": "VEC3"
            },
            # TEXCOORD_0
            {
                "bufferView": 0,
                "byteOffset": 8 * 3 * 4 + 8 * 3 * 4,  # positions + normals
                "componentType": 5126,  # FLOAT
                "count": 8,
                "type": "VEC2"
            },
            # INDICES
            {
                "bufferView": 0,
                "byteOffset": 8 * 3 * 4 + 8 * 3 * 4 + 8 * 2 * 4,  # positions + normals + uvs
                "componentType": 5123,  # UNSIGNED_SHORT
                "count": 36,
                "type": "SCALAR"
            }
        ]
    }

    return gltf, binary_data, indices

def main():
    print("🧪 Creating glTF Demo Cube")

    # Create output directory
    output_dir = Path("test_gltf")
    output_dir.mkdir(exist_ok=True)

    # Generate cube mesh
    gltf_json, binary_data, indices = create_cube_mesh_gltf()

    # Write glTF JSON file
    gltf_path = output_dir / "demo_cube.gltf"
    with open(gltf_path, 'w') as f:
        json.dump(gltf_json, f, indent=2)

    # Write binary data file
    bin_path = output_dir / "cube.bin"
    with open(bin_path, 'wb') as f:
        f.write(binary_data)

    print("✅ Demo glTF files created:")
    print(f"   📄 {gltf_path}")
    print(f"   📦 {bin_path}")
    print(f"   📊 {len(binary_data)} bytes of binary data")
    print(f"   🔺 {len(indices) // 3} triangles")

    print("\n🎉 glTF demo files ready for testing!")
    print("💡 You can view these files in any glTF-compatible viewer")

if __name__ == "__main__":
    main()
