// Browser-compatible Loader Runner
// This module provides a browser-compatible version of webpack's loader runner

export class BrowserLoaderRunner {
  constructor() {
    this.loaders = new Map()
    this.resourcePath = ''
    this.resourceQuery = ''
    this.resourceFragment = ''
    this.context = ''
    this.loaderIndex = 0
    this.async = null
    this.callback = null
  }

  // Register a loader
  registerLoader(name, loader) {
    this.loaders.set(name, loader)
  }

  // Run loaders for a resource
  async runLoaders(options) {
    const {
      resource,
      loaders = [],
      context,
      readResource,
      resourceQuery = '',
      resourceFragment = ''
    } = options

    this.resourcePath = resource
    this.resourceQuery = resourceQuery
    this.resourceFragment = resourceFragment
    this.context = context || ''

    let resourceBuffer = null
    let resourceString = null

    // Read the resource
    if (readResource) {
      try {
        resourceBuffer = await readResource(resource)
        resourceString = resourceBuffer.toString('utf8')
      } catch (error) {
        throw new Error(`Failed to read resource: ${error.message}`)
      }
    }

    // Process through loaders
    let result = resourceString
    let sourceMap = null

    for (let i = 0; i < loaders.length; i++) {
      const loader = loaders[i]
      const loaderName = typeof loader === 'string' ? loader : loader.name || `loader-${i}`

      try {
        const loaderFn = this.loaders.get(loaderName)
        if (!loaderFn) {
          throw new Error(`Loader '${loaderName}' not found`)
        }

        // Create loader context
        const loaderContext = this.createLoaderContext(loaderName, i, loaders.length)

        // Execute loader
        const loaderResult = await this.executeLoader(loaderFn, result, loaderContext)
        
        if (loaderResult) {
          result = loaderResult.content || loaderResult
          if (loaderResult.sourceMap) {
            sourceMap = loaderResult.sourceMap
          }
        }
      } catch (error) {
        throw new Error(`Loader '${loaderName}' failed: ${error.message}`)
      }
    }

    return {
      result,
      resourceBuffer,
      sourceMap,
      cacheable: true
    }
  }

  // Create loader context
  createLoaderContext(loaderName, loaderIndex, loaderCount) {
    const context = {
      // Resource information
      resourcePath: this.resourcePath,
      resourceQuery: this.resourceQuery,
      resourceFragment: this.resourceFragment,
      
      // Loader information
      loaderIndex,
      loaderCount,
      loaderName,
      
      // Context
      context: this.context,
      
      // Async support
      async: () => {
        return (error, result) => {
          if (error) {
            throw error
          }
          return result
        }
      },
      
      // Utilities
      getOptions: (schema) => {
        // In browser environment, we might not have schema validation
        return {}
      },
      
      // Emit file (simplified for browser)
      emitFile: (name, content, sourceMap) => {
        // In browser environment, we might want to store files in memory
        console.log(`Emit file: ${name}`)
        return name
      },
      
      // Add dependency
      addDependency: (dep) => {
        console.log(`Add dependency: ${dep}`)
      },
      
      // Add context dependency
      addContextDependency: (dep) => {
        console.log(`Add context dependency: ${dep}`)
      },
      
      // Add missing dependency
      addMissingDependency: (dep) => {
        console.log(`Add missing dependency: ${dep}`)
      },
      
      // Get dependency
      getDependencies: () => {
        return []
      },
      
      // Get context dependencies
      getContextDependencies: () => {
        return []
      },
      
      // Clear dependencies
      clearDependencies: () => {
        // Clear dependencies
      },
      
      // Resolve
      resolve: (context, request, callback) => {
        // Simplified resolve for browser
        callback(null, request)
      },
      
      // Load module
      loadModule: (request, callback) => {
        // Simplified load module for browser
        callback(null, {})
      },
      
      // Hot module replacement
      hot: false,
      
      // Target
      target: 'web',
      
      // Webpack version
      webpack: true,
      
      // Mode
      mode: 'development'
    }

    return context
  }

  // Execute a loader
  async executeLoader(loaderFn, source, context) {
    return new Promise((resolve, reject) => {
      try {
        // Call the loader function
        const result = loaderFn.call(context, source)
        
        if (result && typeof result.then === 'function') {
          // Async loader
          result.then(resolve).catch(reject)
        } else {
          // Sync loader
          resolve(result)
        }
      } catch (error) {
        reject(error)
      }
    })
  }
}

// Export the loader runner for both CommonJS and ES6 modules
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { BrowserLoaderRunner }
} else if (typeof window !== 'undefined') {
  window.BrowserLoaderRunner = BrowserLoaderRunner
}

// Default export for ES6 modules
export default BrowserLoaderRunner 