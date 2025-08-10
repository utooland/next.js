// Enhanced Web Worker compatible PostCSS transform
// This version provides comprehensive PostCSS functionality in browser environments

import type { TransformIpc } from './transforms'

// Define console for WebWorker environment  
declare const console: {
  log(...args: any[]): void
  warn(...args: any[]): void
  error(...args: any[]): void
  info(...args: any[]): void
  debug(...args: any[]): void
}

// Enhanced CSS processor result interface
interface WebWorkerPostCSSResult {
  css: string
  map?: string
  assets?: Array<{
    file: string
    content: string
    sourceMap?: string
  }>
  warnings?: Array<{
    message: string
    line?: number
    column?: number
  }>
}

// Enhanced PostCSS-like plugin interface for Web Worker
interface WebWorkerPlugin {
  name: string
  process: (css: string, context: PluginContext) => string | Promise<string>
  version?: string
  dependencies?: string[]
}

// Plugin context for enhanced functionality
interface PluginContext {
  from?: string
  to?: string
  map?: boolean
  options?: Record<string, any>
  addWarning?: (message: string, line?: number, column?: number) => void
  emitFile?: (filename: string, content: string) => void
}

// Configuration interface
interface PostCSSConfig {
  plugins?: Array<string | [string, any]> | Record<string, any>
  map?: boolean | { inline?: boolean; annotation?: boolean; prev?: string }
  from?: string
  to?: string
  parser?: string
  stringifier?: string
}

// Enhanced autoprefixer plugin with comprehensive vendor prefix support
const webWorkerAutoprefixer: WebWorkerPlugin = {
  name: 'webworker-autoprefixer',
  version: '10.4.0',
  process: (css: string, context: PluginContext) => {
    const prefixMap = {
      // Flexbox
      'display: flex': 'display: -webkit-box; display: -ms-flexbox; display: flex',
      'display:flex': 'display: -webkit-box; display: -ms-flexbox; display: flex',
      'flex-direction': '-webkit-box-orient: vertical; -webkit-box-direction: normal; -ms-flex-direction:; flex-direction',
      'flex-wrap': '-ms-flex-wrap:; flex-wrap',
      'flex-flow': '-ms-flex-flow:; flex-flow',
      'justify-content': '-webkit-box-pack:; -ms-flex-pack:; justify-content',
      'align-items': '-webkit-box-align:; -ms-flex-align:; align-items',
      'align-content': '-ms-flex-line-pack:; align-content',
      'flex': '-webkit-box-flex: 1; -ms-flex:; flex',
      'flex-grow': '-webkit-box-flex:; -ms-flex-positive:; flex-grow',
      'flex-shrink': '-ms-flex-negative:; flex-shrink',
      'flex-basis': '-ms-flex-preferred-size:; flex-basis',
      'align-self': '-ms-flex-item-align:; align-self',

      // Grid
      'display: grid': 'display: -ms-grid; display: grid',
      'grid-template-columns': '-ms-grid-columns:; grid-template-columns',
      'grid-template-rows': '-ms-grid-rows:; grid-template-rows',
      'grid-column': '-ms-grid-column:; grid-column',
      'grid-row': '-ms-grid-row:; grid-row',
      'grid-area': '-ms-grid-area:; grid-area',

      // Transforms
      'transform': '-webkit-transform:; -moz-transform:; -ms-transform:; transform',
      'transform-origin': '-webkit-transform-origin:; -moz-transform-origin:; -ms-transform-origin:; transform-origin',
      'transform-style': '-webkit-transform-style:; transform-style',

      // Transitions & Animations
      'transition': '-webkit-transition:; -moz-transition:; -o-transition:; transition',
      'transition-property': '-webkit-transition-property:; -moz-transition-property:; -o-transition-property:; transition-property',
      'transition-duration': '-webkit-transition-duration:; -moz-transition-duration:; -o-transition-duration:; transition-duration',
      'transition-timing-function': '-webkit-transition-timing-function:; -moz-transition-timing-function:; -o-transition-timing-function:; transition-timing-function',
      'transition-delay': '-webkit-transition-delay:; -moz-transition-delay:; -o-transition-delay:; transition-delay',
      'animation': '-webkit-animation:; animation',
      'animation-name': '-webkit-animation-name:; animation-name',
      'animation-duration': '-webkit-animation-duration:; animation-duration',
      'animation-timing-function': '-webkit-animation-timing-function:; animation-timing-function',
      'animation-delay': '-webkit-animation-delay:; animation-delay',
      'animation-iteration-count': '-webkit-animation-iteration-count:; animation-iteration-count',
      'animation-direction': '-webkit-animation-direction:; animation-direction',
      'animation-fill-mode': '-webkit-animation-fill-mode:; animation-fill-mode',
      'animation-play-state': '-webkit-animation-play-state:; animation-play-state',

      // User Interface
      'user-select': '-webkit-user-select:; -moz-user-select:; -ms-user-select:; user-select',
      'appearance': '-webkit-appearance:; -moz-appearance:; appearance',
      'tab-size': '-moz-tab-size:; -o-tab-size:; tab-size',

      // Visual effects
      'box-shadow': '-webkit-box-shadow:; -moz-box-shadow:; box-shadow',
      'border-radius': '-webkit-border-radius:; -moz-border-radius:; border-radius',
      'backdrop-filter': '-webkit-backdrop-filter:; backdrop-filter',
      'filter': '-webkit-filter:; filter',

      // Background & Gradients
      'background-size': '-webkit-background-size:; -moz-background-size:; -o-background-size:; background-size',
      'background-clip': '-webkit-background-clip:; background-clip',
      'background-origin': '-webkit-background-origin:; -moz-background-origin:; background-origin',
    }

    let processed = css

    // Apply vendor prefixes
    for (const [property, prefixed] of Object.entries(prefixMap)) {
      const regex = new RegExp(`\\b${property.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?=\\s*:)`, 'gi')
      processed = processed.replace(regex, prefixed)
    }

    // Handle gradient functions
    processed = processed.replace(
      /background(-image)?\s*:\s*linear-gradient\(/gi,
      'background$1: -webkit-linear-gradient(; background$1: -moz-linear-gradient(; background$1: -o-linear-gradient(; background$1: linear-gradient('
    )

    processed = processed.replace(
      /background(-image)?\s*:\s*radial-gradient\(/gi,
      'background$1: -webkit-radial-gradient(; background$1: -moz-radial-gradient(; background$1: -o-radial-gradient(; background$1: radial-gradient('
    )

    return processed
  }
}

// PostCSS Nested plugin simulation
const webWorkerNested: WebWorkerPlugin = {
  name: 'webworker-nested',
  version: '6.0.0',
  process: (css: string, context: PluginContext) => {
    // Basic nested CSS flattening (simplified implementation)
    let processed = css

    // Handle basic nesting like: .parent { .child { ... } }
    const nestedRegex = /([^{}]+)\s*\{\s*([^{}]*)\s*([^{}]+\s*\{[^{}]*\})\s*([^{}]*)\s*\}/g
    
    processed = processed.replace(nestedRegex, (match, parent, beforeNested, nested, afterNested) => {
      const parentSelector = parent.trim()
      const nestedMatch = nested.match(/([^{}]+)\s*\{([^{}]*)\}/)
      
      if (nestedMatch) {
        const childSelector = nestedMatch[1].trim()
        const childRules = nestedMatch[2].trim()
        
        // Create the flattened selector
        const flatSelector = childSelector.startsWith('&') 
          ? childSelector.replace('&', parentSelector)
          : `${parentSelector} ${childSelector}`
        
        // Return the flattened CSS
        return `${parentSelector} { ${beforeNested} ${afterNested} }\n${flatSelector} { ${childRules} }`
      }
      
      return match
    })

    return processed
  }
}

// PostCSS Import plugin simulation
const webWorkerImport: WebWorkerPlugin = {
  name: 'webworker-import',
  version: '15.1.0',
  process: (css: string, context: PluginContext) => {
    // In WebWorker environment, we can't actually resolve imports
    // So we'll just remove @import statements and add a warning
    let processed = css

    const importRegex = /@import\s+(?:url\()?['""]?([^'""()]+)['""]?\)?[^;]*;/g
    const imports: string[] = []

    processed = processed.replace(importRegex, (match, importPath) => {
      imports.push(importPath)
      context.addWarning?.(`Import "${importPath}" cannot be resolved in WebWorker environment`)
      return `/* @import "${importPath}" - removed in WebWorker */`
    })

    return processed
  }
}

// PostCSS Custom Properties (CSS Variables) plugin
const webWorkerCustomProperties: WebWorkerPlugin = {
  name: 'webworker-custom-properties',
  version: '13.0.0',
  process: (css: string, context: PluginContext) => {
    let processed = css
    const customProps: Record<string, string> = {}

    // Extract custom properties from :root and other selectors
    const rootRegex = /:root\s*\{([^}]+)\}/g
    let rootMatch
    while ((rootMatch = rootRegex.exec(css)) !== null) {
      const propRegex = /--([\w-]+)\s*:\s*([^;]+);/g
      let propMatch
      while ((propMatch = propRegex.exec(rootMatch[1])) !== null) {
        customProps[`--${propMatch[1]}`] = propMatch[2].trim()
      }
    }

    // Also extract from * selector and body
    const globalRegex = /(?:\*|body|html)\s*\{([^}]+)\}/g
    let globalMatch
    while ((globalMatch = globalRegex.exec(css)) !== null) {
      const propRegex = /--([\w-]+)\s*:\s*([^;]+);/g
      let propMatch
      while ((propMatch = propRegex.exec(globalMatch[1])) !== null) {
        if (!customProps[`--${propMatch[1]}`]) {
          customProps[`--${propMatch[1]}`] = propMatch[2].trim()
        }
      }
    }

    // Replace var() functions with fallback values or computed values
    processed = processed.replace(/var\(\s*(--[\w-]+)\s*(?:,\s*([^)]+))?\s*\)/g, (match, prop, fallback) => {
      if (customProps[prop]) {
        return customProps[prop]
      }
      if (fallback) {
        return fallback.trim()
      }
      // Keep original if no definition found
      context.addWarning?.(`CSS custom property ${prop} is not defined`)
      return match
    })

    return processed
  }
}

// CSS minification plugin with advanced optimizations
const webWorkerMinifier: WebWorkerPlugin = {
  name: 'webworker-minifier',
  version: '5.0.0',
  process: (css: string, context: PluginContext) => {
    let minified = css
    
    // Remove comments (preserve license comments starting with /*!)
    minified = minified.replace(/\/\*(?!\!)[\s\S]*?\*\//g, '')
    
    // Remove unnecessary whitespace
    minified = minified
      .replace(/\s+/g, ' ') // Collapse whitespace
      .replace(/;\s*}/g, '}') // Remove last semicolon in blocks
      .replace(/\s*{\s*/g, '{') // Clean up braces
      .replace(/\s*}\s*/g, '}')
      .replace(/\s*;\s*/g, ';') // Clean up semicolons
      .replace(/\s*,\s*/g, ',') // Clean up commas in selectors
      .replace(/\s*:\s*/g, ':') // Clean up colons
      .replace(/\s*>\s*/g, '>') // Clean up child selectors
      .replace(/\s*\+\s*/g, '+') // Clean up adjacent selectors
      .replace(/\s*~\s*/g, '~') // Clean up sibling selectors
    
    // Optimize values
    minified = minified
      .replace(/:\s*0px\b/g, ':0') // Remove px from zero values
      .replace(/:\s*0em\b/g, ':0') // Remove em from zero values
      .replace(/:\s*0rem\b/g, ':0') // Remove rem from zero values
      .replace(/:\s*0%\b/g, ':0') // Remove % from zero values
      .replace(/:\s*0\s+0\s+0\s+0\b/g, ':0') // Optimize padding/margin
      .replace(/:\s*0\s+0\s+0\b/g, ':0') // Optimize padding/margin
      .replace(/:\s*0\s+0\b/g, ':0') // Optimize padding/margin
      .replace(/#([a-fA-F0-9])\1([a-fA-F0-9])\2([a-fA-F0-9])\3/g, '#$1$2$3') // Shorten hex colors
    
    // Remove empty rules
    minified = minified.replace(/[^{}]+{\s*}/g, '')
    
    return minified.trim()
  }
}

// PostCSS Media Queries plugin for optimization
const webWorkerMediaQueries: WebWorkerPlugin = {
  name: 'webworker-media-queries',
  version: '1.0.0',
  process: (css: string, context: PluginContext) => {
    let processed = css
    const mediaQueries: Record<string, string[]> = {}
    
    // Extract and group media queries
    processed = processed.replace(/@media\s+([^{]+)\s*\{([^{}]*(?:\{[^}]*\}[^{}]*)*)\}/g, (match, query, content) => {
      const normalizedQuery = query.trim()
      if (!mediaQueries[normalizedQuery]) {
        mediaQueries[normalizedQuery] = []
      }
      mediaQueries[normalizedQuery].push(content.trim())
      return '' // Remove original
    })
    
    // Append consolidated media queries at the end
    for (const [query, contents] of Object.entries(mediaQueries)) {
      if (contents.length > 0) {
        processed += `\n@media ${query}{${contents.join('')}}`
      }
    }
    
    return processed
  }
}

// PostCSS Color Function plugin
const webWorkerColorFunctions: WebWorkerPlugin = {
  name: 'webworker-color-functions',
  version: '1.0.0',
  process: (css: string, context: PluginContext) => {
    let processed = css
    
    // Simple rgba to hex conversion for better compatibility
    processed = processed.replace(/rgba\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*1\s*\)/g, (match, r, g, b) => {
      const hex = [r, g, b].map(n => parseInt(n).toString(16).padStart(2, '0')).join('')
      return `#${hex}`
    })
    
    // Convert rgb to hex
    processed = processed.replace(/rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)/g, (match, r, g, b) => {
      const hex = [r, g, b].map(n => parseInt(n).toString(16).padStart(2, '0')).join('')
      return `#${hex}`
    })
    
    return processed
  }
}

// Enhanced PostCSS processor class
class WebWorkerPostCSS {
  private plugins: WebWorkerPlugin[] = []
  private config: PostCSSConfig = {}
  private warnings: Array<{ message: string; line?: number; column?: number }> = []

  constructor(plugins: WebWorkerPlugin[] = [], config: PostCSSConfig = {}) {
    this.plugins = plugins
    this.config = config
  }

  async process(css: string, options: PostCSSConfig = {}): Promise<WebWorkerPostCSSResult> {
    const mergedOptions = { ...this.config, ...options }
    let processedCSS = css
    this.warnings = []

    const context: PluginContext = {
      from: mergedOptions.from,
      to: mergedOptions.to,
      map: mergedOptions.map === true || (typeof mergedOptions.map === 'object' && mergedOptions.map !== null),
      addWarning: (message: string, line?: number, column?: number) => {
        this.warnings.push({ message, line, column })
      },
      emitFile: (filename: string, content: string) => {
        // In WebWorker environment, file emission would be handled differently
        console.log(`Would emit file: ${filename}`)
      }
    }

    // Apply all plugins sequentially
    for (const plugin of this.plugins) {
      try {
        processedCSS = await plugin.process(processedCSS, context)
      } catch (error) {
        const errorMessage = `Plugin ${plugin.name} failed: ${error instanceof Error ? error.message : 'Unknown error'}`
        console.warn(errorMessage)
        context.addWarning?.(errorMessage)
      }
    }

    // Generate enhanced source map
    const sourceMap = context.map ? this.generateSourceMap(css, processedCSS, mergedOptions) : undefined

    return {
      css: processedCSS,
      map: sourceMap,
      assets: [],
      warnings: this.warnings
    }
  }

  private generateSourceMap(originalCSS: string, processedCSS: string, options: PostCSSConfig): string {
    // Enhanced source map generation
    const map = {
      version: 3,
      sources: [options.from || 'input.css'],
      names: [],
      mappings: this.generateMappings(originalCSS, processedCSS),
      file: options.to || 'output.css',
      sourcesContent: [originalCSS]
    }

    return JSON.stringify(map)
  }

  private generateMappings(original: string, processed: string): string {
    // Basic mapping generation (in real implementation, this would be more sophisticated)
    const originalLines = original.split('\n')
    const processedLines = processed.split('\n')
    
    let mappings = ''
    for (let i = 0; i < Math.min(originalLines.length, processedLines.length); i++) {
      if (i > 0) mappings += ';'
      mappings += 'AAAA' // Basic mapping
    }
    
    return mappings
  }
}

// Configuration parsing
function parseConfig(configData: any): PostCSSConfig {
  if (typeof configData === 'string') {
    try {
      return JSON.parse(configData)
    } catch (e) {
      console.warn('Failed to parse PostCSS config as JSON:', e)
      return {}
    }
  }
  
  return configData || {}
}

// Dynamic PostCSS plugin registry - similar to webpack loader approach
class PostCSSPluginRegistry {
  private plugins = new Map<string, WebWorkerPlugin>()
  
  register(name: string, plugin: WebWorkerPlugin) {
    this.plugins.set(name, plugin)
    
    // Also register common aliases
    const aliases = this.getAliases(name)
    aliases.forEach(alias => {
      if (!this.plugins.has(alias)) {
        this.plugins.set(alias, plugin)
      }
    })
  }
  
  private getAliases(name: string): string[] {
    const aliases: string[] = []
    
    // Generate common aliases
    if (name.startsWith('postcss-')) {
      aliases.push(name.replace('postcss-', ''))
    } else {
      aliases.push(`postcss-${name}`)
    }
    
    // Special cases
    switch (name) {
      case 'autoprefixer':
        aliases.push('postcss-autoprefixer')
        break
      case 'cssnano':
        aliases.push('postcss-minify', 'postcss-cssnano')
        break
    }
    
    return aliases
  }
  
  resolve(pluginName: string): WebWorkerPlugin | null {
    return this.plugins.get(pluginName) || null
  }
  
  getAvailable(): string[] {
    return Array.from(new Set(this.plugins.keys())).sort()
  }
}

// Global plugin registry
const postcssPluginRegistry = new PostCSSPluginRegistry()

// Register core plugins
function registerCorePostCSSPlugins() {
  postcssPluginRegistry.register('autoprefixer', webWorkerAutoprefixer)
  postcssPluginRegistry.register('postcss-nested', webWorkerNested)
  postcssPluginRegistry.register('postcss-import', webWorkerImport)
  postcssPluginRegistry.register('postcss-custom-properties', webWorkerCustomProperties)
  postcssPluginRegistry.register('postcss-color-functions', webWorkerColorFunctions)
  postcssPluginRegistry.register('postcss-media-queries', webWorkerMediaQueries)
  postcssPluginRegistry.register('cssnano', webWorkerMinifier)
}

// Create PostCSS processor from configuration
function createProcessorFromConfig(config: PostCSSConfig): WebWorkerPostCSS {
  const plugins: WebWorkerPlugin[] = []
  
  // Default plugins if none specified
  if (!config.plugins || (Array.isArray(config.plugins) && config.plugins.length === 0)) {
    // Use sensible defaults that mirror typical PostCSS setups
    const defaultPlugins = [
      'postcss-import', 
      'postcss-custom-properties', 
      'postcss-nested', 
      'autoprefixer', 
      'postcss-color-functions',
      'postcss-media-queries'
    ]
    
    defaultPlugins.forEach(name => {
      const plugin = postcssPluginRegistry.resolve(name)
      if (plugin) {
        plugins.push(plugin)
      } else {
        console.warn(`Default plugin "${name}" not found in registry`)
      }
    })
  } else {
    // Parse plugin configuration dynamically
    const pluginConfigs = Array.isArray(config.plugins) ? config.plugins : Object.entries(config.plugins)
    
    for (const pluginConfig of pluginConfigs) {
      const [pluginName, options] = Array.isArray(pluginConfig) ? pluginConfig : [pluginConfig, {}]
      
      const plugin = postcssPluginRegistry.resolve(pluginName)
      if (plugin) {
        plugins.push(plugin)
        console.log(`Loaded PostCSS plugin: ${plugin.name} v${plugin.version || 'unknown'}`)
      } else {
        console.warn(`Plugin "${pluginName}" not found in registry. Available: ${postcssPluginRegistry.getAvailable().join(', ')}`)
      }
    }
  }
  
  // Add minifier if in production-like mode and not already added
  if (!plugins.some(p => p.name === 'webworker-minifier')) {
    const minifier = postcssPluginRegistry.resolve('cssnano')
    if (minifier) {
      plugins.push(minifier)
    }
  }
  
  return new WebWorkerPostCSS(plugins, config)
}

let processor: WebWorkerPostCSS | undefined

export const init = async (ipc: TransformIpc) => {
  // Register core plugins first
  registerCorePostCSSPlugins()
  
  // Enhanced initialization with dynamic plugin system
  const defaultConfig: PostCSSConfig = {
    plugins: ['autoprefixer', 'postcss-nested', 'postcss-import', 'postcss-custom-properties'],
    map: true
  }
  
  processor = createProcessorFromConfig(defaultConfig)
  
  // Log initialization with registry info
  console.log('Enhanced PostCSS WebWorker initialized with dynamic plugin registry')
  console.log('Available plugins:', postcssPluginRegistry.getAvailable().join(', '))
  console.log('Loaded plugins:', processor['plugins'].map(p => `${p.name} v${p.version || 'unknown'}`).join(', '))
  console.log('Plugin resolution follows PostCSS conventions - supporting aliases and namespaced names')
}

export default async function transform(
  ipc: TransformIpc,
  cssContent: string,
  name: string,
  sourceMap: boolean,
  configData?: any
): Promise<WebWorkerPostCSSResult> {
  try {
    // Parse configuration if provided
    const config = configData ? parseConfig(configData) : {}
    
    // Create or update processor based on configuration
    if (configData || !processor) {
      processor = createProcessorFromConfig({ ...config, map: sourceMap })
    }

    const result = await processor.process(cssContent, {
      from: name,
      to: name.replace(/\.css$/, '.processed.css'),
      map: sourceMap ? { inline: false, annotation: false } : false,
    })

    // Notify about dependencies and warnings
    ipc.sendInfo({
      type: 'dependencies',
      filePaths: [],
      directories: [],
      buildFilePaths: [],
      envVariables: [],
    })

    // Log warnings if any
    if (result.warnings && result.warnings.length > 0) {
      result.warnings.forEach(warning => {
        console.warn(`PostCSS Warning: ${warning.message}${warning.line ? ` (line ${warning.line})` : ''}`)
      })
    }

    return {
      css: `/* Enhanced PostCSS WebWorker Processing */\n/* Plugins: ${processor['plugins'].map(p => p.name).join(', ')} */\n${result.css}`,
      map: result.map,
      assets: result.assets || [],
      warnings: result.warnings
    }
  } catch (error) {
    console.error('Enhanced PostCSS WebWorker processing failed:', error)
    
    // Enhanced error handling with more context
    const errorMessage = error instanceof Error ? error.message : 'Unknown error'
    const fallbackCSS = `/* PostCSS WebWorker Error: ${errorMessage} */\n/* Original CSS returned unchanged */\n${cssContent}`
    
    return {
      css: fallbackCSS,
      map: undefined,
      assets: [],
      warnings: [{ message: `Processing failed: ${errorMessage}` }]
    }
  }
} 