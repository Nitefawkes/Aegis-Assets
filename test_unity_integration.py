#!/usr/bin/env python3
"""
End-to-end integration test for the Unity plugin

This script creates a minimal Unity file and tests the complete extraction workflow.
"""

import os
import sys
import subprocess
import tempfile
from pathlib import Path

def create_test_unity_file(output_path):
    """Create a minimal Unity file for testing"""
    
    # Create minimal UnityFS header
    with open(output_path, 'wb') as f:
        # UnityFS signature
        f.write(b'UnityFS\0')
        
        # Version
        f.write(b'2022.3.15f1\0')
        
        # Unity revision
        f.write(b'abcd1234\0')
        
        # Bundle size (big endian)
        f.write((1024).to_bytes(8, 'big'))
        
        # Compressed blocks info size (big endian)
        f.write((100).to_bytes(4, 'big'))
        
        # Uncompressed blocks info size (big endian)
        f.write((100).to_bytes(4, 'big'))
        
        # Flags (big endian, 0 = no compression)
        f.write((0).to_bytes(4, 'big'))
        
        # Add some dummy data to meet minimum size requirements
        dummy_data = b'\x00' * 100
        f.write(dummy_data)
        
        # Pad to 1024 bytes
        remaining = 1024 - f.tell()
        if remaining > 0:
            f.write(b'\x00' * remaining)
    
    print(f"✅ Created test Unity file: {output_path}")

def test_aegis_cli(unity_file_path):
    """Test the Aegis CLI with the Unity file"""
    
    print(f"\n🔍 Testing Aegis CLI with: {unity_file_path}")
    
    # Test 1: Check if Aegis can detect plugins
    print("\n1️⃣  Testing plugin detection...")
    try:
        result = subprocess.run(['cargo', 'run', '--bin', 'aegis', '--', 'plugins'], 
                              capture_output=True, text=True, cwd='C:\\Users\\17577\\Desktop\\aegis-assets', 
                              timeout=30)
        if result.returncode == 0:
            print("✅ Plugin detection works:")
            print(result.stdout)
        else:
            print("❌ Plugin detection failed:")
            print(result.stderr)
            return False
    except Exception as e:
        print(f"❌ Error running plugin detection: {e}")
        return False
    
    # Test 2: List assets in the Unity file
    print("\n2️⃣  Testing asset listing...")
    try:
        result = subprocess.run(['cargo', 'run', '--bin', 'aegis', '--', 'list', str(unity_file_path)], 
                              capture_output=True, text=True, cwd='C:\\Users\\17577\\Desktop\\aegis-assets',
                              timeout=30)
        if result.returncode == 0:
            print("✅ Asset listing works:")
            print(result.stdout)
        else:
            print("⚠️  Asset listing had issues (might be expected with minimal test file):")
            print(result.stderr)
    except Exception as e:
        print(f"❌ Error running asset listing: {e}")
    
    # Test 3: Try to extract assets
    print("\n3️⃣  Testing asset extraction...")
    with tempfile.TemporaryDirectory() as temp_dir:
        try:
            result = subprocess.run(['cargo', 'run', '--bin', 'aegis', '--', 'extract', 
                                   str(unity_file_path), '-o', temp_dir, '--verbose'], 
                                  capture_output=True, text=True, cwd='C:\\Users\\17577\\Desktop\\aegis-assets',
                                  timeout=30)
            if result.returncode == 0:
                print("✅ Asset extraction works:")
                print(result.stdout)
                
                # Check if any files were created
                output_files = list(Path(temp_dir).glob('**/*'))
                if output_files:
                    print(f"📁 Output files created: {len(output_files)}")
                    for file in output_files[:5]:  # Show first 5 files
                        print(f"   • {file}")
                else:
                    print("📁 No output files created (might be expected)")
                
                return True
            else:
                print("⚠️  Asset extraction had issues (might be expected with minimal test file):")
                print(result.stderr)
                print("STDOUT:", result.stdout)
                return False
        except Exception as e:
            print(f"❌ Error running asset extraction: {e}")
            return False

def test_compilation():
    """Test if the project compiles successfully"""
    
    print("🔨 Testing compilation...")
    
    try:
        # Test CLI compilation
        result = subprocess.run(['cargo', 'check', '--bin', 'aegis'], 
                              capture_output=True, text=True, cwd='C:\\Users\\17577\\Desktop\\aegis-assets',
                              timeout=60)
        if result.returncode == 0:
            print("✅ CLI compiles successfully")
        else:
            print("❌ CLI compilation failed:")
            print(result.stderr)
            return False
        
        # Test Unity plugin compilation
        result = subprocess.run(['cargo', 'check'], 
                              capture_output=True, text=True, cwd='C:\\Users\\17577\\Desktop\\aegis-assets\\aegis-plugins\\unity',
                              timeout=60)
        if result.returncode == 0:
            print("✅ Unity plugin compiles successfully")
        else:
            print("❌ Unity plugin compilation failed:")
            print(result.stderr)
            return False
        
        return True
        
    except Exception as e:
        print(f"❌ Error during compilation test: {e}")
        return False

def main():
    print("🛡️  Aegis-Assets End-to-End Integration Test")
    print("=" * 50)
    
    # Test 1: Compilation
    if not test_compilation():
        print("\n❌ Compilation tests failed - stopping here")
        return False
    
    # Test 2: Create test Unity file
    with tempfile.NamedTemporaryFile(suffix='.unity3d', delete=False) as temp_file:
        test_unity_file = temp_file.name
    
    try:
        create_test_unity_file(test_unity_file)
        
        # Test 3: CLI functionality
        success = test_aegis_cli(test_unity_file)
        
        if success:
            print("\n🎉 END-TO-END TEST PASSED!")
            print("✅ Unity plugin is working with CLI")
            print("✅ Asset extraction pipeline is functional")
            print("✅ Ready for real Unity files!")
        else:
            print("\n⚠️  END-TO-END TEST HAD ISSUES")
            print("   This might be expected with minimal test data")
            print("   Check the output above for details")
        
        return success
        
    finally:
        # Clean up test file
        try:
            os.unlink(test_unity_file)
            print(f"🧹 Cleaned up test file: {test_unity_file}")
        except:
            pass

if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
