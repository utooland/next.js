/** @type {import('next').NextConfig} */
const nextConfig = {
  // 启用 Turbopack
  experimental: {
    turbo: {
      rules: {
        '*.css': {
          loaders: ['browser-postcss-loader'],
          as: '*.css'
        }
      }
    }
  }
}

module.exports = nextConfig 