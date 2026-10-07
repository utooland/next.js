// Web worker globals - no Node.js worker_threads available
// @ts-ignore
if (typeof globalThis.process === 'undefined') {
  // @ts-ignore
  globalThis.process = {} as any
}

// @ts-ignore
process.turbopack = {}
