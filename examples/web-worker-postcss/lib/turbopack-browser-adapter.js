// Turbopack Browser Adapter
// This module adapts Turbopack for browser environments

export class TurbopackBrowserAdapter {
  constructor(options = {}) {
    this.options = {
      useWorkers: true,
      workerPoolSize: 4,
      ...options
    }
    
    this.workerPool = []
    this.loaderRunner = null
    this.registeredLoaders = new Map()
    
    this.init()
  }

  // Initialize the adapter
  async init() {
    // Initialize loader runner
    let BrowserLoaderRunner
    if (window.BrowserLoaderRunner) {
      BrowserLoaderRunner = window.BrowserLoaderRunner
    } else {
      const module = await import('./browser-loader-runner.js')
      BrowserLoaderRunner = module.BrowserLoaderRunner || module.default
    }
    this.loaderRunner = new BrowserLoaderRunner()
    
    // Register default loaders
    await this.registerDefaultLoaders()
    
    // Initialize worker pool if needed
    if (this.options.useWorkers && typeof Worker !== 'undefined') {
      await this.initWorkerPool()
    }
  }

  // Register default loaders
  async registerDefaultLoaders() {
    // Register PostCSS loader
    let PostCssLoader
    if (window.BrowserPostCssLoader) {
      PostCssLoader = window.BrowserPostCssLoader
    } else {
      const module = await import('./browser-postcss-loader.js')
      PostCssLoader = module.BrowserPostCssLoader || module.default
    }
    this.loaderRunner.registerLoader('browser-postcss-loader', (source, context) => {
      const loader = new PostCssLoader({
        sourceMap: true,
        useWorker: this.options.useWorkers
      })
      return loader.apply(source, context)
    })

    // Register CSS loader
    this.loaderRunner.registerLoader('css-loader', (source, context) => {
      // Simple CSS loader that just returns the source
      return source
    })

    // Register style loader
    this.loaderRunner.registerLoader('style-loader', (source, context) => {
      // Create a style tag and inject the CSS
      const styleId = `style-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`
      const styleElement = document.createElement('style')
      styleElement.id = styleId
      styleElement.textContent = source
      document.head.appendChild(styleElement)
      
      return `/* Injected by style-loader */`
    })
  }

  // Initialize worker pool
  async initWorkerPool() {
    for (let i = 0; i < this.options.workerPoolSize; i++) {
      const worker = new Worker('/turbopack-worker.js')
      this.workerPool.push({
        worker,
        busy: false,
        id: i
      })
    }
  }

  // Get available worker
  getAvailableWorker() {
    const availableWorker = this.workerPool.find(w => !w.busy)
    if (availableWorker) {
      availableWorker.busy = true
      return availableWorker
    }
    return null
  }

  // Release worker
  releaseWorker(worker) {
    worker.busy = false
  }

  // Process module with loaders
  async processModule(modulePath, source, loaders = []) {
    // Check if we should use worker
    if (this.options.useWorkers && this.workerPool.length > 0) {
      return this.processModuleInWorker(modulePath, source, loaders)
    } else {
      return this.processModuleInMainThread(modulePath, source, loaders)
    }
  }

  // Process module in worker
  async processModuleInWorker(modulePath, source, loaders) {
    const workerInfo = this.getAvailableWorker()
    if (!workerInfo) {
      // Fallback to main thread if no workers available
      return this.processModuleInMainThread(modulePath, source, loaders)
    }

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.releaseWorker(workerInfo)
        reject(new Error('Module processing timeout'))
      }, 30000)

      workerInfo.worker.onmessage = (event) => {
        clearTimeout(timeout)
        const { type, data, error } = event.data

        if (type === 'result') {
          this.releaseWorker(workerInfo)
          resolve(data)
        } else if (type === 'error') {
          this.releaseWorker(workerInfo)
          reject(new Error(error))
        }
      }

      workerInfo.worker.onerror = (error) => {
        clearTimeout(timeout)
        this.releaseWorker(workerInfo)
        reject(error)
      }

      // Send processing request
      workerInfo.worker.postMessage({
        type: 'process-module',
        modulePath,
        source,
        loaders
      })
    })
  }

  // Process module in main thread
  async processModuleInMainThread(modulePath, source, loaders) {
    try {
      const result = await this.loaderRunner.runLoaders({
        resource: modulePath,
        loaders: loaders,
        context: '',
        readResource: async () => Buffer.from(source, 'utf8')
      })

      return {
        source: result.result,
        sourceMap: result.sourceMap,
        dependencies: []
      }
    } catch (error) {
      throw new Error(`Failed to process module ${modulePath}: ${error.message}`)
    }
  }

  // Register a custom loader
  registerLoader(name, loader) {
    this.loaderRunner.registerLoader(name, loader)
    this.registeredLoaders.set(name, loader)
  }

  // Get registered loaders
  getRegisteredLoaders() {
    return Array.from(this.registeredLoaders.keys())
  }

  // Process CSS file
  async processCssFile(filePath, source) {
    const loaders = ['browser-postcss-loader', 'css-loader']
    return this.processModule(filePath, source, loaders)
  }

  // Process CSS and inject into DOM
  async processAndInjectCss(filePath, source) {
    const loaders = ['browser-postcss-loader', 'css-loader', 'style-loader']
    return this.processModule(filePath, source, loaders)
  }

  // Watch for file changes
  watchFiles(files, callback) {
    // In browser environment, we might use File System Access API or other methods
    // For now, we'll simulate file watching
    console.log('File watching not implemented in browser environment')
    
    // Return a cleanup function
    return () => {
      console.log('Stopping file watch')
    }
  }

  // Build project
  async build(options = {}) {
    const {
      entry,
      output,
      loaders = {},
      plugins = []
    } = options

    console.log('Building project with Turbopack Browser Adapter...')
    
    // Process entry files
    const results = []
    
    for (const [name, filePath] of Object.entries(entry)) {
      try {
        // In a real implementation, you would read the file
        const source = await this.readFile(filePath)
        const result = await this.processModule(filePath, source, loaders[name] || [])
        
        results.push({
          name,
          filePath,
          result
        })
      } catch (error) {
        console.error(`Failed to process entry ${name}:`, error)
      }
    }
    
    return results
  }

  // Read file (simplified for browser)
  async readFile(filePath) {
    // In a real implementation, you would fetch the file
    // For now, we'll return a placeholder
    return `/* CSS file: ${filePath} */`
  }

  // Cleanup resources
  cleanup() {
    // Terminate workers
    this.workerPool.forEach(({ worker }) => {
      worker.terminate()
    })
    this.workerPool = []
  }
}

// Export the adapter for both CommonJS and ES6 modules
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { TurbopackBrowserAdapter }
} else if (typeof window !== 'undefined') {
  window.TurbopackBrowserAdapter = TurbopackBrowserAdapter
}

// Default export for ES6 modules
export default TurbopackBrowserAdapter 