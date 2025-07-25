#!/bin/bash

# Web Worker PostCSS Example Runner
# This script sets up and runs the example

set -e

echo "🚀 Setting up Web Worker PostCSS Example..."

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    echo "❌ Node.js is not installed. Please install Node.js 18+ first."
    exit 1
fi

# Check Node.js version
NODE_VERSION=$(node -v | cut -d'v' -f2 | cut -d'.' -f1)
if [ "$NODE_VERSION" -lt 18 ]; then
    echo "❌ Node.js version 18+ is required. Current version: $(node -v)"
    exit 1
fi

echo "✅ Node.js version: $(node -v)"

# Check if we're in the right directory
if [ ! -f "package.json" ]; then
    echo "❌ package.json not found. Please run this script from the example directory."
    exit 1
fi

# Install dependencies
echo "📦 Installing dependencies..."
npm install

# Create necessary directories if they don't exist
mkdir -p public
mkdir -p pages
mkdir -p components
mkdir -p styles

echo "✅ Dependencies installed successfully!"

# Check if all required files exist
REQUIRED_FILES=(
    "pages/index.tsx"
    "components/PostCssProcessor.tsx"
    "public/postcss-worker.js"
    "styles/Home.module.css"
    "styles/PostCssProcessor.module.css"
    "next.config.js"
    "postcss.config.js"
    "tailwind.config.js"
)

echo "🔍 Checking required files..."
for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo "✅ $file"
    else
        echo "❌ $file - Missing!"
        exit 1
    fi
done

echo ""
echo "🎉 Setup complete! Starting development server..."
echo ""
echo "📱 Open your browser and navigate to: http://localhost:3000"
echo "🛑 Press Ctrl+C to stop the server"
echo ""

# Start the development server
npm run dev 