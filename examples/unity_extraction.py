#!/usr/bin/env python3
"""
Aegis Unity Plugin Example

This script demonstrates how to use the Unity plugin to extract and convert 
Unity assets from game files.
"""

import os
import sys
import subprocess
from pathlib import Path

def find_unity_files(directory):
    """Find Unity asset files in a directory"""
    unity_extensions = ['.unity3d', '.assets', '.sharedAssets', '.resource', '.resS']
    unity_files = []
    
    for ext in unity_extensions:
        unity_files.extend(Path(directory).rglob(f"*{ext}"))
    
    return unity_files

def extract_unity_assets(unity_file, output_dir):
    """Extract assets from a Unity file using Aegis"""
    
    print(f"Processing: {unity_file}")
    
    # Build the aegis command
    cmd = [
        "aegis",           # Aegis CLI tool
        "extract",         # Extract command
        str(unity_file),   # Input file
        "-o", str(output_dir),  # Output directory
        "--plugin", "unity",    # Use Unity plugin
        "--convert",           # Convert assets to standard formats
        "--format", "png",     # Convert textures to PNG
        "--format", "gltf",    # Convert meshes to glTF
        "--format", "ogg",     # Convert audio to OGG
        "--compliance-check",  # Check compliance before extraction
        "--verbose"           # Verbose output
    ]
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        print("✅ Extraction successful!")
        print(f"Output: {result.stdout}")
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Extraction failed: {e}")
        print(f"Error output: {e.stderr}")
        return False
    except FileNotFoundError:
        print("❌ Aegis CLI not found. Make sure it's installed and in PATH.")
        return False

def list_unity_assets(unity_file):
    """List assets in a Unity file without extracting"""
    
    cmd = [
        "aegis",
        "list",
        str(unity_file),
        "--plugin", "unity",
        "--details"
    ]
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        print(f"Assets in {unity_file}:")
        print(result.stdout)
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Failed to list assets: {e}")
        return False
    except FileNotFoundError:
        print("❌ Aegis CLI not found.")
        return False

def check_compliance(unity_file):
    """Check compliance status of a Unity file"""
    
    cmd = [
        "aegis",
        "compliance",
        str(unity_file),
        "--plugin", "unity"
    ]
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        print(f"Compliance check for {unity_file}:")
        print(result.stdout)
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Compliance check failed: {e}")
        return False

def main():
    if len(sys.argv) < 2:
        print("Usage:")
        print(f"  {sys.argv[0]} <command> <path> [output_dir]")
        print()
        print("Commands:")
        print("  find <directory>     - Find Unity files in directory")
        print("  list <file>          - List assets in Unity file") 
        print("  extract <file> [out] - Extract assets from Unity file")
        print("  compliance <file>    - Check compliance status")
        print()
        print("Examples:")
        print(f"  {sys.argv[0]} find ./game_data/")
        print(f"  {sys.argv[0]} list sharedassets0.assets")
        print(f"  {sys.argv[0]} extract level1.unity3d ./extracted/")
        print(f"  {sys.argv[0]} compliance globalgamemanagers.assets")
        sys.exit(1)
    
    command = sys.argv[1]
    
    if command == "find":
        if len(sys.argv) < 3:
            print("Error: find command requires directory path")
            sys.exit(1)
        
        directory = sys.argv[2]
        print(f"Searching for Unity files in: {directory}")
        
        unity_files = find_unity_files(directory)
        if unity_files:
            print(f"\nFound {len(unity_files)} Unity files:")
            for file in unity_files:
                print(f"  📁 {file}")
        else:
            print("No Unity files found.")
    
    elif command == "list":
        if len(sys.argv) < 3:
            print("Error: list command requires file path")
            sys.exit(1)
        
        unity_file = Path(sys.argv[2])
        if not unity_file.exists():
            print(f"Error: File not found: {unity_file}")
            sys.exit(1)
        
        list_unity_assets(unity_file)
    
    elif command == "extract":
        if len(sys.argv) < 3:
            print("Error: extract command requires file path")
            sys.exit(1)
        
        unity_file = Path(sys.argv[2])
        if not unity_file.exists():
            print(f"Error: File not found: {unity_file}")
            sys.exit(1)
        
        output_dir = Path(sys.argv[3]) if len(sys.argv) > 3 else Path("./extracted")
        output_dir.mkdir(parents=True, exist_ok=True)
        
        extract_unity_assets(unity_file, output_dir)
    
    elif command == "compliance":
        if len(sys.argv) < 3:
            print("Error: compliance command requires file path")
            sys.exit(1)
        
        unity_file = Path(sys.argv[2])
        if not unity_file.exists():
            print(f"Error: File not found: {unity_file}")
            sys.exit(1)
        
        check_compliance(unity_file)
    
    else:
        print(f"Error: Unknown command '{command}'")
        print("Use --help to see available commands")
        sys.exit(1)

if __name__ == "__main__":
    main()
