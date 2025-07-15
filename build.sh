#!/bin/bash

echo "🦀 Building Rust Terminal Emulator..."

# Build in release mode
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo "📁 Executable location: target/release/terminal-app"
    echo "🚀 To run: ./target/release/terminal-app"
else
    echo "❌ Build failed!"
    exit 1
fi
