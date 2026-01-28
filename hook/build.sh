#!/bin/bash
set -e

echo "Building hook library..."
make clean
make

echo "✓ Hook library built successfully: libhook.so"
echo ""
echo "To use the hook library, set the environment variable:"
echo "  export C2RUST_HOOK_LIB=$(pwd)/libhook.so"
