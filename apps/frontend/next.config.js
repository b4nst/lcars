/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  output: 'export', // Static SPA export - no Node.js server required
  images: {
    unoptimized: true,
  },
  // For SPA behavior with client-side routing
  trailingSlash: true,
}

module.exports = nextConfig
