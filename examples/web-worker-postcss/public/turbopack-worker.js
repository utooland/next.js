// Turbopack Web Worker
// This worker handles module processing for Turbopack in browser environments

// Import required modules (in a real implementation, these would be loaded from CDN or bundled)
let BrowserLoaderRunner = null
let BrowserPostCssLoader = null

// Load required modules
async function loadModules() {
  if (!BrowserLoaderRunner) {
    // In a real implementation, you would import these modules
    // For now, we'll simulate the modules
    BrowserLoaderRunner = class {
      constructor() {
        this.loaders = new Map()
      }
      
      registerLoader(name, loader) {
        this.loaders.set(name, loader)
      }
      
      async runLoaders(options) {
        const { resource, loaders = [], readResource } = options
        
        let source = await readResource()
        if (typeof source === 'string') {
          source = Buffer.from(source, 'utf8')
        }
        
        let result = source.toString('utf8')
        
        for (const loaderName of loaders) {
          const loader = this.loaders.get(loaderName)
          if (loader) {
            const context = createLoaderContext(resource, loaderName)
            result = await executeLoader(loader, result, context)
          }
        }
        
        return {
          result,
          sourceMap: null,
          cacheable: true
        }
      }
    }
  }
  
  if (!BrowserPostCssLoader) {
    BrowserPostCssLoader = class {
      constructor(options = {}) {
        this.options = options
      }
      
      async apply(source, context) {
        const callback = context.async()
        
        try {
          // Simulate PostCSS processing
          const processedCss = await this.processCss(source)
          callback(null, processedCss)
        } catch (error) {
          callback(error)
        }
      }
      
      async processCss(css) {
        // Simulate processing time
        await new Promise(resolve => setTimeout(resolve, 50))
        
        let processedCss = css
        
        // Apply Autoprefixer
        processedCss = this.applyAutoprefixer(processedCss)
        
        // Apply Tailwind CSS
        processedCss = this.applyTailwindCss(processedCss)
        
        return processedCss
      }
      
      applyAutoprefixer(css) {
        css = css.replace(/display:\s*flex/g, 'display: -webkit-flex; display: -ms-flexbox; display: flex')
        css = css.replace(/flex-direction:\s*column/g, '-webkit-flex-direction: column; -ms-flex-direction: column; flex-direction: column')
        css = css.replace(/align-items:\s*center/g, '-webkit-align-items: center; -ms-flex-align: center; align-items: center')
        css = css.replace(/justify-content:\s*center/g, '-webkit-justify-content: center; -ms-flex-pack: center; justify-content: center')
        css = css.replace(/transform:\s*([^;]+);/g, 'transform: $1; -webkit-transform: $1; -ms-transform: $1;')
        css = css.replace(/transition:\s*([^;]+);/g, 'transition: $1; -webkit-transition: $1; -o-transition: $1;')
        
        return css
      }
      
      applyTailwindCss(css) {
        css = css.replace(/@tailwind\s+(\w+);/g, '/* Tailwind $1 styles */')
        
        css = css.replace(/@apply\s+([^;]+);/g, (match, classes) => {
          const classMap = {
            'bg-primary-500': 'background-color: #3b82f6',
            'text-white': 'color: white',
            'px-4': 'padding-left: 1rem; padding-right: 1rem',
            'py-2': 'padding-top: 0.5rem; padding-bottom: 0.5rem',
            'rounded-lg': 'border-radius: 0.5rem',
            'hover:bg-primary-900': 'background-color: #1e3a8a',
            'transition-colors': 'transition: color 0.15s ease-in-out, background-color 0.15s ease-in-out',
            'bg-white': 'background-color: white',
            'shadow-lg': 'box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05)',
            'p-6': 'padding: 1.5rem',
            'border': 'border-width: 1px',
            'border-gray-200': 'border-color: #e5e7eb'
          }
          
          return classes.split(' ').map(cls => classMap[cls] || cls).join('; ')
        })
        
        return css
      }
    }
  }
}

// Create loader context
function createLoaderContext(resource, loaderName) {
  return {
    resourcePath: resource,
    async: () => {
      return (error, result) => {
        if (error) {
          throw error
        }
        return result
      }
    },
    getOptions: () => ({}),
    emitFile: (name, content) => name,
    addDependency: (dep) => {},
    addContextDependency: (dep) => {},
    addMissingDependency: (dep) => {},
    resolve: (context, request, callback) => callback(null, request),
    loadModule: (request, callback) => callback(null, {}),
    hot: false,
    target: 'web',
    webpack: true,
    mode: 'development'
  }
}

// Execute loader
async function executeLoader(loader, source, context) {
  return new Promise((resolve, reject) => {
    try {
      const result = loader.call(context, source)
      
      if (result && typeof result.then === 'function') {
        result.then(resolve).catch(reject)
      } else {
        resolve(result)
      }
    } catch (error) {
      reject(error)
    }
  })
}

// Initialize loader runner
let loaderRunner = null

async function initLoaderRunner() {
  await loadModules()
  
  loaderRunner = new BrowserLoaderRunner()
  
  // Register PostCSS loader
  loaderRunner.registerLoader('browser-postcss-loader', (source, context) => {
    const loader = new BrowserPostCssLoader({
      sourceMap: true
    })
    return loader.apply(source, context)
  })
  
  // Register CSS loader
  loaderRunner.registerLoader('css-loader', (source, context) => {
    return source
  })
  
  // Register style loader
  loaderRunner.registerLoader('style-loader', (source, context) => {
    return `/* Style loader processed: ${source.length} characters */`
  })
}

// Handle messages from main thread
self.addEventListener('message', async (event) => {
  const { type, modulePath, source, loaders } = event.data
  
  if (type === 'process-module') {
    try {
      // Initialize loader runner if not already done
      if (!loaderRunner) {
        await initLoaderRunner()
      }
      
      console.log(`Processing module: ${modulePath} with loaders:`, loaders)
      
      const result = await loaderRunner.runLoaders({
        resource: modulePath,
        loaders: loaders,
        context: '',
        readResource: async () => Buffer.from(source, 'utf8')
      })
      
      self.postMessage({
        type: 'result',
        data: {
          source: result.result,
          sourceMap: result.sourceMap,
          dependencies: []
        }
      })
    } catch (error) {
      console.error('Error processing module:', error)
      self.postMessage({
        type: 'error',
        error: error.message
      })
    }
  }
})

// Error handling
self.addEventListener('error', (event) => {
  console.error('Web Worker error:', event.error)
  self.postMessage({
    type: 'error',
    error: event.error.message
  })
})

// Unhandled promise rejections
self.addEventListener('unhandledrejection', (event) => {
  console.error('Unhandled promise rejection in Web Worker:', event.reason)
  self.postMessage({
    type: 'error',
    error: event.reason.message || 'Unhandled promise rejection'
  })
})

// Log worker initialization
console.log('Turbopack Web Worker initialized') 