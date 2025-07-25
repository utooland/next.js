// Browser-compatible PostCSS Loader
// This loader runs PostCSS in a Web Worker or main thread

export class BrowserPostCssLoader {
  constructor(options = {}) {
    this.options = {
      sourceMap: false,
      plugins: [],
      config: null,
      ...options
    }
  }

  // Main loader function
  async apply(source, context) {
    const callback = context.async()
    
    try {
      // Get PostCSS configuration
      const config = await this.getPostCssConfig(context)
      
      // Process CSS with PostCSS
      const result = await this.processWithPostCss(source, config, context)
      
      // Return the processed CSS
      callback(null, result.css, result.map)
    } catch (error) {
      callback(error)
    }
  }

  // Get PostCSS configuration
  async getPostCssConfig(context) {
    if (this.options.config) {
      return this.options.config
    }

    // Try to load config from various sources
    const configSources = [
      'postcss.config.js',
      'postcss.config.mjs',
      'postcss.config.cjs',
      '.postcssrc',
      '.postcssrc.json',
      '.postcssrc.yaml',
      '.postcssrc.yml'
    ]

    for (const configFile of configSources) {
      try {
        // In browser environment, we might need to fetch the config
        const config = await this.loadConfig(configFile, context)
        if (config) {
          return config
        }
      } catch (error) {
        // Continue to next config file
        continue
      }
    }

    // Return default config
    return {
      plugins: {
        'autoprefixer': {
          overrideBrowserslist: ['> 1%', 'last 2 versions', 'not dead']
        }
      }
    }
  }

  // Load configuration file
  async loadConfig(configFile, context) {
    // In browser environment, we might need to fetch the config file
    // For now, we'll return a default configuration
    return {
      plugins: {
        'autoprefixer': {
          overrideBrowserslist: ['> 1%', 'last 2 versions', 'not dead']
        }
      }
    }
  }

  // Process CSS with PostCSS
  async processWithPostCss(source, config, context) {
    // Check if we should use Web Worker
    const useWorker = typeof Worker !== 'undefined' && this.options.useWorker !== false

    if (useWorker) {
      return this.processInWorker(source, config, context)
    } else {
      return this.processInMainThread(source, config, context)
    }
  }

  // Process CSS in Web Worker
  async processInWorker(source, config, context) {
    return new Promise((resolve, reject) => {
      const worker = new Worker('/postcss-worker.js')
      
      const timeout = setTimeout(() => {
        worker.terminate()
        reject(new Error('PostCSS processing timeout'))
      }, 30000) // 30 second timeout

      worker.onmessage = (event) => {
        clearTimeout(timeout)
        const { type, data, error } = event.data

        if (type === 'result') {
          resolve({
            css: data.css,
            map: data.map
          })
        } else if (type === 'error') {
          reject(new Error(error))
        }
        
        worker.terminate()
      }

      worker.onerror = (error) => {
        clearTimeout(timeout)
        reject(error)
        worker.terminate()
      }

      // Send processing request to worker
      worker.postMessage({
        type: 'process',
        css: source,
        config: config,
        sourceMap: this.options.sourceMap,
        resourcePath: context.resourcePath
      })
    })
  }

  // Process CSS in main thread
  async processInMainThread(source, config, context) {
    // Load PostCSS dynamically
    const postcss = await this.loadPostCss()
    
    // Build plugin list
    const plugins = this.buildPluginList(config.plugins)
    
    // Process CSS
    const result = await postcss(plugins).process(source, {
      from: context.resourcePath,
      to: context.resourcePath,
      map: this.options.sourceMap ? { inline: false } : false
    })

    return {
      css: result.css,
      map: result.map ? result.map.toString() : undefined
    }
  }

  // Load PostCSS
  async loadPostCss() {
    // In a real implementation, you would load PostCSS from CDN or bundle
    // For this example, we'll simulate PostCSS functionality
    
    return (plugins) => ({
      process: async (css, options) => {
        // Simulate processing time
        await new Promise(resolve => setTimeout(resolve, 100))
        
        let processedCss = css
        
        // Apply plugins
        for (const plugin of plugins) {
          if (plugin.postcssPlugin === 'autoprefixer') {
            processedCss = this.applyAutoprefixer(processedCss)
          } else if (plugin.postcssPlugin === 'tailwindcss') {
            processedCss = this.applyTailwindCss(processedCss)
          }
        }
        
        return {
          css: processedCss,
          map: options.map ? '/* source map */' : null
        }
      }
    })
  }

  // Build plugin list
  buildPluginList(plugins) {
    const pluginList = []
    
    for (const [name, options] of Object.entries(plugins)) {
      if (name === 'autoprefixer') {
        pluginList.push(this.createAutoprefixerPlugin(options))
      } else if (name === 'tailwindcss') {
        pluginList.push(this.createTailwindCssPlugin(options))
      }
    }
    
    return pluginList
  }

  // Create Autoprefixer plugin
  createAutoprefixerPlugin(options) {
    return {
      postcssPlugin: 'autoprefixer',
      options: options
    }
  }

  // Create Tailwind CSS plugin
  createTailwindCssPlugin(options) {
    return {
      postcssPlugin: 'tailwindcss',
      options: options
    }
  }

  // Apply Autoprefixer
  applyAutoprefixer(css) {
    // Add vendor prefixes
    css = css.replace(/display:\s*flex/g, 'display: -webkit-flex; display: -ms-flexbox; display: flex')
    css = css.replace(/flex-direction:\s*column/g, '-webkit-flex-direction: column; -ms-flex-direction: column; flex-direction: column')
    css = css.replace(/align-items:\s*center/g, '-webkit-align-items: center; -ms-flex-align: center; align-items: center')
    css = css.replace(/justify-content:\s*center/g, '-webkit-justify-content: center; -ms-flex-pack: center; justify-content: center')
    css = css.replace(/transform:\s*([^;]+);/g, 'transform: $1; -webkit-transform: $1; -ms-transform: $1;')
    css = css.replace(/transition:\s*([^;]+);/g, 'transition: $1; -webkit-transition: $1; -o-transition: $1;')
    
    return css
  }

  // Apply Tailwind CSS
  applyTailwindCss(css) {
    // Process @tailwind directives
    css = css.replace(/@tailwind\s+(\w+);/g, '/* Tailwind $1 styles */')
    
    // Process @apply directives
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

// Export the loader for both CommonJS and ES6 modules
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { BrowserPostCssLoader }
} else if (typeof window !== 'undefined') {
  window.BrowserPostCssLoader = BrowserPostCssLoader
}

// Default export for ES6 modules
export default BrowserPostCssLoader 