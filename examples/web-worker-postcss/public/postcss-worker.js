// Web Worker for PostCSS processing
// This file is served from the public directory

// Import PostCSS and plugins using dynamic imports
let postcss = null
let autoprefixer = null
let tailwindcss = null

// Load PostCSS and plugins
async function loadPostCss() {
  if (!postcss) {
    try {
      // In a real implementation, you would load these from CDN or bundle them
      // For this example, we'll simulate the loading
      console.log('Loading PostCSS in Web Worker...')
      
      // Simulate loading PostCSS
      postcss = {
        process: async (css, options) => {
          // Simulate processing time
          await new Promise(resolve => setTimeout(resolve, 50))
          
          let processedCss = css
          
          // Simulate Tailwind CSS processing
          processedCss = processedCss.replace(/@tailwind\s+(\w+);/g, '/* Tailwind $1 styles */')
          
          // Simulate Autoprefixer processing
          processedCss = processedCss.replace(/display:\s*flex/g, 'display: -webkit-flex; display: -ms-flexbox; display: flex')
          processedCss = processedCss.replace(/flex-direction:\s*column/g, '-webkit-flex-direction: column; -ms-flex-direction: column; flex-direction: column')
          processedCss = processedCss.replace(/align-items:\s*center/g, '-webkit-align-items: center; -ms-flex-align: center; align-items: center')
          processedCss = processedCss.replace(/justify-content:\s*center/g, '-webkit-justify-content: center; -ms-flex-pack: center; justify-content: center')
          processedCss = processedCss.replace(/transform:\s*([^;]+);/g, 'transform: $1; -webkit-transform: $1; -ms-transform: $1;')
          processedCss = processedCss.replace(/transition:\s*([^;]+);/g, 'transition: $1; -webkit-transition: $1; -o-transition: $1;')
          
          // Simulate @apply directive processing
          processedCss = processedCss.replace(/@apply\s+([^;]+);/g, (match, classes) => {
            // Convert Tailwind classes to actual CSS
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
          
          return {
            css: processedCss,
            map: options.map ? '/* source map */' : null
          }
        }
      }
      
      console.log('PostCSS loaded successfully in Web Worker')
    } catch (error) {
      console.error('Failed to load PostCSS in Web Worker:', error)
      throw error
    }
  }
}

// Process PostCSS
async function processPostCss(css, config, sourceMap) {
  await loadPostCss()
  
  if (!postcss) {
    throw new Error('PostCSS not available in Web Worker')
  }
  
  console.log('Processing CSS in Web Worker...')
  
  // Process the CSS
  const result = await postcss.process(css, {
    from: undefined,
    to: undefined,
    map: sourceMap ? { inline: false } : false
  })
  
  console.log('CSS processing completed in Web Worker')
  
  return {
    css: result.css,
    map: result.map ? result.map.toString() : undefined,
    assets: []
  }
}

// Handle messages from main thread
self.addEventListener('message', async (event) => {
  const { type, css, config, sourceMap } = event.data
  
  if (type === 'process') {
    try {
      console.log('Received CSS processing request in Web Worker')
      const result = await processPostCss(css, config, sourceMap)
      
      self.postMessage({
        type: 'result',
        data: result
      })
    } catch (error) {
      console.error('Error processing CSS in Web Worker:', error)
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
console.log('PostCSS Web Worker initialized') 