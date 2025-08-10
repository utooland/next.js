import { type TransformIpc } from './transforms'

// Define Buffer for WebWorker environment
interface BufferLike {
  toString(encoding?: string): string
}

declare const Buffer: {
  from(data: string | Uint8Array, encoding?: string): BufferLike
}

// Define console for WebWorker environment  
declare const console: {
  log(...args: any[]): void
  warn(...args: any[]): void
  error(...args: any[]): void
  info(...args: any[]): void
  debug(...args: any[]): void
  time(label?: string): void
  timeEnd(label?: string): void
}

// Enhanced loader configuration interface
interface LoaderConfig {
  loader: string
  options?: Record<string, any>
  ident?: string
}

// Enhanced loader context with comprehensive webpack-like API
interface LoaderContext {
  resource: string
  resourcePath: string
  resourceQuery: string
  resourceFragment: string
  query: Record<string, any>
  loaderIndex: number
  loaders: LoaderConfig[]
  
  // Callback functions
  callback: (err: Error | null, result?: string | { toString(encoding?: string): string }, sourceMap?: any, meta?: any) => void
  async: () => (err: Error | null, result?: string | { toString(encoding?: string): string }, sourceMap?: any, meta?: any) => void
  
  // Utility functions
  getOptions: () => Record<string, any>
  emitWarning: (warning: string | Error) => void
  emitError: (error: string | Error) => void
  emitFile: (name: string, content: string | { toString(encoding?: string): string }, sourceMap?: any) => void
  
  // Logging
  getLogger: (name?: string) => Logger
  
  // Dependencies
  addDependency: (file: string) => void
  addContextDependency: (context: string) => void
  addMissingDependency: (missing: string) => void
  addBuildDependency: (file: string) => void
  
  // Caching
  cacheable: (flag?: boolean) => void
  
  // Path utilities
  resolve: (context: string, request: string, callback: (err: Error | null, result?: string) => void) => void
  
  // Hot module replacement
  hot: boolean
  
  // Mode and target
  mode: 'development' | 'production' | 'none'
  target: string
  
  // Webpack version
  version: string
  
  // Root context
  rootContext: string
  context: string
  
  // File system access
  fs: any
  
  // Source map
  sourceMap: boolean
}

// Enhanced logger interface
interface Logger {
  error: (...args: any[]) => void
  warn: (...args: any[]) => void
  info: (...args: any[]) => void
  log: (...args: any[]) => void
  debug: (...args: any[]) => void
  trace: (...args: any[]) => void
  group: (...args: any[]) => void
  groupEnd: () => void
  groupCollapsed: (...args: any[]) => void
  profile: (label?: string) => void
  profileEnd: (label?: string) => void
  time: (label?: string) => void
  timeEnd: (label?: string) => void
  clear: () => void
  status: (...args: any[]) => void
}

// Loader result interface
interface LoaderResult {
  content: string | BufferLike
  sourceMap?: any
  meta?: any
  dependencies?: string[]
  contextDependencies?: string[]
  buildDependencies?: string[]
  cacheable?: boolean
}

// Enhanced loader runner for WebWorker environment
function runLoadersWebWorker(
  options: {
    resource: string
    loaders: LoaderConfig[]
    readResource: (filename: string, callback: (err: Error | null, buffer?: { toString(encoding?: string): string }) => void) => void
  },
  callback: (err: Error | null, result?: LoaderResult) => void
) {
  const { resource, loaders, readResource } = options
  
  const resourceParts = resource.split('?')
  const resourcePath = resourceParts[0]
  const resourceQuery = resourceParts[1] || ''
  const resourceFragment = resourceParts[2] || ''
  
  readResource(resourcePath, async (err, buffer) => {
    if (err) {
      callback(err)
      return
    }
    
    if (!buffer) {
      callback(new Error('Failed to read resource'))
      return
    }
    
    let content: string | Buffer = buffer.toString('utf8')
    let sourceMap: any = null
    let meta: any = {}
    const dependencies: string[] = []
    const contextDependencies: string[] = []
    const buildDependencies: string[] = []
    let cacheable = true
    
    try {
      // Execute loader chain in reverse order (webpack convention)
      for (let i = loaders.length - 1; i >= 0; i--) {
        const loaderConfig = loaders[i]
        
        // Create enhanced loader context
        const context: LoaderContext = {
          resource,
          resourcePath,
          resourceQuery,
          resourceFragment,
          query: loaderConfig.options || {},
          loaderIndex: i,
          loaders,
          
          // Callback functions
          callback: () => {}, // Will be overridden for async loaders
          async: () => () => {}, // Async support
          
          // Utility functions
          getOptions: () => loaderConfig.options || {},
          emitWarning: (warning: string | Error) => {
            const message = warning instanceof Error ? warning.message : warning
            console.warn(`[${loaderConfig.loader}] Warning: ${message}`)
          },
          emitError: (error: string | Error) => {
            const message = error instanceof Error ? error.message : error
            console.error(`[${loaderConfig.loader}] Error: ${message}`)
          },
          emitFile: (name: string, content: string | Buffer, sourceMap?: any) => {
            console.log(`[${loaderConfig.loader}] Would emit file: ${name}`)
          },
          
          // Logging
          getLogger: (name?: string) => createLogger(loaderConfig.loader, name),
          
          // Dependencies
          addDependency: (file: string) => dependencies.push(file),
          addContextDependency: (context: string) => contextDependencies.push(context),
          addMissingDependency: (missing: string) => {
            console.warn(`Missing dependency: ${missing}`)
          },
          addBuildDependency: (file: string) => buildDependencies.push(file),
          
          // Caching
          cacheable: (flag: boolean = true) => { cacheable = flag },
          
          // Path utilities
          resolve: (context: string, request: string, callback: (err: Error | null, result?: string) => void) => {
            // Simplified resolve in WebWorker environment
            callback(null, request)
          },
          
          // Environment info
          hot: false, // HMR not available in WebWorker
          mode: 'production' as const,
          target: 'webworker',
          version: '5.0.0-webworker',
          rootContext: '/',
          context: resourcePath.split('/').slice(0, -1).join('/'),
          fs: null, // File system not available in WebWorker
          sourceMap: true
        }
        
        // Apply the loader
        const result = await applyEnhancedLoader(loaderConfig.loader, content, context)
        
        if (result instanceof Error) {
          callback(result)
          return
        }
        
        // Update content and metadata
        if (typeof result === 'object' && result !== null) {
          content = result.content || content
          if (result.sourceMap) sourceMap = result.sourceMap
          if (result.meta) meta = { ...meta, ...result.meta }
        } else {
          content = result
        }
      }
      
      callback(null, {
        content,
        sourceMap,
        meta,
        dependencies,
        contextDependencies,
        buildDependencies,
        cacheable
      })
    } catch (error) {
      callback(error as Error)
    }
  })
}

// Create enhanced logger
function createLogger(loaderName: string, name?: string): Logger {
  const prefix = `[${loaderName}${name ? `:${name}` : ''}]`
  
  return {
    error: (...args) => console.error(prefix, ...args),
    warn: (...args) => console.warn(prefix, ...args),
    info: (...args) => console.info(prefix, ...args),
    log: (...args) => console.log(prefix, ...args),
    debug: (...args) => console.debug(prefix, ...args),
    trace: (...args) => console.log(prefix, 'TRACE:', ...args),
    group: (...args) => console.log(prefix, 'GROUP:', ...args),
    groupEnd: () => console.log(prefix, 'GROUP_END'),
    groupCollapsed: (...args) => console.log(prefix, 'GROUP_COLLAPSED:', ...args),
    profile: (label?: string) => console.log(prefix, 'PROFILE:', label),
    profileEnd: (label?: string) => console.log(prefix, 'PROFILE_END:', label),
    time: (label?: string) => console.time(`${prefix} ${label}`),
    timeEnd: (label?: string) => console.timeEnd(`${prefix} ${label}`),
    clear: () => console.log(prefix, 'CLEAR'),
    status: (...args) => console.log(prefix, 'STATUS:', ...args)
  }
}

// Dynamic loader registry - mirroring native webpack loader behavior
class LoaderRegistry {
  private loaders = new Map<string, LoaderImplementation>()
  
  register(name: string, implementation: LoaderImplementation) {
    this.loaders.set(name, implementation)
  }
  
  resolve(loaderName: string): LoaderImplementation | null {
    // Try exact match first
    if (this.loaders.has(loaderName)) {
      return this.loaders.get(loaderName)!
    }
    
    // Try base name matching (like native webpack)
    const baseName = loaderName.split('/').pop()?.replace(/\..+$/, '') || loaderName
    
    // Check various possible loader names
    const candidates = [
      baseName,
      `${baseName}-loader`,
      baseName.replace('-loader', ''),
    ]
    
    for (const candidate of candidates) {
      if (this.loaders.has(candidate)) {
        return this.loaders.get(candidate)!
      }
    }
    
    return null
  }
  
  getAvailable(): string[] {
    return Array.from(this.loaders.keys())
  }
}

interface LoaderImplementation {
  name: string
  version?: string
  process: (content: string | BufferLike, context: LoaderContext) => Promise<string | BufferLike | LoaderResult>
}

// Global loader registry
const loaderRegistry = new LoaderRegistry()

// Dynamically apply loader based on registry
async function applyEnhancedLoader(
  loaderName: string, 
  content: string | Buffer, 
  context: LoaderContext
): Promise<string | Buffer | LoaderResult | Error> {
  const logger = context.getLogger()
  
  try {
    // Try to resolve the loader from registry
    const loaderImpl = loaderRegistry.resolve(loaderName)
    
    if (loaderImpl) {
      logger.info(`Processing with ${loaderImpl.name} v${loaderImpl.version || 'unknown'}`)
      return await loaderImpl.process(content, context)
    }
    
    // Fallback to identity loader
    logger.warn(`Loader "${loaderName}" not found in registry, using identity loader`)
    return await identityLoader(content, context)
    
  } catch (error) {
    logger.error(`Failed to apply loader ${loaderName}: ${error}`)
    return error instanceof Error ? error : new Error(String(error))
  }
}

// Identity loader as fallback
async function identityLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  return {
    content: typeof content === 'string' ? content : content.toString('utf8'),
    meta: { processed: false, loader: 'identity' }
  }
}

// Enhanced Babel loader implementation
async function applyBabelLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const source = typeof content === 'string' ? content : content.toString('utf8')
  const options = context.getOptions()
  
  let transformed = source
  
  // Basic JSX transformation
  if (options.presets?.includes('@babel/preset-react') || context.resourcePath.endsWith('.jsx')) {
    transformed = transformed
      .replace(/React\.createElement\(/g, '/*#__PURE__*/ React.createElement(')
      .replace(/import\s+React/g, 'import React')
  }
  
  // Basic ES6+ transformations
  if (options.presets?.includes('@babel/preset-env')) {
    // Transform arrow functions
    transformed = transformed.replace(
      /const\s+(\w+)\s*=\s*\(([^)]*)\)\s*=>\s*{/g,
      'const $1 = function($2) {'
    )
    
    // Transform template literals (basic)
    transformed = transformed.replace(
      /`([^`]*\$\{[^}]*\}[^`]*)`/g,
      (match, template) => {
        return '"' + template.replace(/\$\{([^}]+)\}/g, '" + ($1) + "') + '"'
      }
    )
  }
  
  return {
    content: `/* Babel WebWorker Loader */\n${transformed}`,
    sourceMap: context.sourceMap ? generateBasicSourceMap(context.resourcePath, source, transformed) : undefined,
    meta: { babel: { version: '7.0.0-webworker' } }
  }
}

// Enhanced CSS loader implementation
async function applyCssLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const css = typeof content === 'string' ? content : content.toString('utf8')
  const options = context.getOptions()
  
  // Process CSS imports
  let processedCss = css
  const imports: string[] = []
  
  processedCss = processedCss.replace(
    /@import\s+(?:url\()?['""]?([^'""()]+)['""]?\)?[^;]*;/g,
    (match, importPath) => {
      imports.push(importPath)
      context.addDependency(importPath)
      return `/* @import "${importPath}" */`
    }
  )
  
  // CSS Modules support
  if (options.modules || context.resourcePath.includes('.module.')) {
    const moduleResult = processCssModules(processedCss, context.resourcePath)
    return {
      content: `module.exports = ${JSON.stringify(moduleResult.exports)};\nmodule.exports._css = ${JSON.stringify(moduleResult.css)};`,
      meta: { cssModules: moduleResult.exports }
    }
  }
  
  // Regular CSS to JS module
  return {
    content: `module.exports = ${JSON.stringify(processedCss)};`,
    meta: { imports }
  }
}

// Enhanced Style loader implementation
async function applyStyleLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const css = typeof content === 'string' ? content : content.toString('utf8')
  
  const styleInject = `
function injectStyle(css) {
  if (typeof document !== 'undefined') {
    const style = document.createElement('style');
    style.textContent = css;
    document.head.appendChild(style);
  }
}

const css = ${JSON.stringify(css)};
injectStyle(css);
module.exports = {};
`
  
  return {
    content: `/* Style WebWorker Loader */\n${styleInject}`,
    meta: { injected: true }
  }
}

// TypeScript loader implementation
async function applyTypeScriptLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const source = typeof content === 'string' ? content : content.toString('utf8')
  
  let transformed = source
  
  // Basic type stripping
  transformed = transformed
    .replace(/:\s*(string|number|boolean|any|void|null|undefined)\b/g, '')
    .replace(/\?\s*:/g, ':')
    .replace(/interface\s+\w+\s*\{[^}]*\}/g, '')
    .replace(/type\s+\w+\s*=\s*[^;]+;/g, '')
    .replace(/enum\s+\w+\s*\{[^}]*\}/g, '')
    .replace(/declare\s+[^;]+;/g, '')
    .replace(/export\s+type\s+[^;]+;/g, '')
    .replace(/import\s+type\s+[^;]+;/g, '')
  
  return {
    content: `/* TypeScript WebWorker Loader */\n${transformed}`,
    sourceMap: context.sourceMap ? generateBasicSourceMap(context.resourcePath, source, transformed) : undefined,
    meta: { typescript: { transpiled: true } }
  }
}

// Vue SFC loader implementation
async function applyVueLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const source = typeof content === 'string' ? content : content.toString('utf8')
  
  // Basic Vue SFC parsing
  const templateMatch = source.match(/<template[^>]*>([\s\S]*?)<\/template>/)
  const scriptMatch = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)
  const styleMatch = source.match(/<style[^>]*>([\s\S]*?)<\/style>/)
  
  const template = templateMatch ? templateMatch[1].trim() : ''
  const script = scriptMatch ? scriptMatch[1].trim() : 'export default {}'
  const style = styleMatch ? styleMatch[1].trim() : ''
  
  const vueComponent = `
${script.replace('export default', 'const component =')}

if (component.template === undefined) {
  component.template = ${JSON.stringify(template)};
}

if (${JSON.stringify(style)}) {
  const style = document.createElement('style');
  style.textContent = ${JSON.stringify(style)};
  document.head.appendChild(style);
}

export default component;
`
  
  return {
    content: `/* Vue WebWorker Loader */\n${vueComponent}`,
    meta: { vue: { sfc: true } }
  }
}

// Sass/SCSS loader implementation
async function applySassLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const scss = typeof content === 'string' ? content : content.toString('utf8')
  
  // Basic SCSS processing (very simplified)
  let processed = scss
  
  // Process variables (basic)
  const variables: Record<string, string> = {}
  processed = processed.replace(/\$([a-zA-Z_][\w-]*)\s*:\s*([^;]+);/g, (match, name, value) => {
    variables[name] = value.trim()
    return `/* $${name}: ${value} */`
  })
  
  // Replace variable usage
  for (const [name, value] of Object.entries(variables)) {
    processed = processed.replace(new RegExp(`\\$${name}\\b`, 'g'), value)
  }
  
  // Process nesting (basic)
  processed = processed.replace(
    /([^{}]+)\s*\{\s*([^{}]*)\s*([^{}]+\s*\{[^}]*\})\s*([^{}]*)\s*\}/g,
    (match, parent, beforeNested, nested, afterNested) => {
      const parentSelector = parent.trim()
      const nestedMatch = nested.match(/([^{}]+)\s*\{([^{}]*)\}/)
      
      if (nestedMatch) {
        const childSelector = nestedMatch[1].trim()
        const childRules = nestedMatch[2].trim()
        
        const flatSelector = childSelector.startsWith('&') 
          ? childSelector.replace('&', parentSelector)
          : `${parentSelector} ${childSelector}`
        
        return `${parentSelector} { ${beforeNested} ${afterNested} }\n${flatSelector} { ${childRules} }`
      }
      
      return match
    }
  )
  
  return {
    content: processed,
    meta: { sass: { processed: true } }
  }
}

// Additional helper functions

function processCssModules(css: string, resourcePath: string): { css: string; exports: Record<string, string> } {
  const exports: Record<string, string> = {}
  let processedCss = css
  
  // Generate hashed class names
  const classNames = css.match(/\.([a-zA-Z_][\w-]*)/g) || []
  
  classNames.forEach(className => {
    const originalName = className.slice(1)
    const hashedName = `${originalName}_${generateHash(resourcePath + originalName)}`
    exports[originalName] = hashedName
    processedCss = processedCss.replace(new RegExp(`\\.${originalName}\\b`, 'g'), `.${hashedName}`)
  })
  
  return { css: processedCss, exports }
}

function generateHash(input: string): string {
  let hash = 0
  for (let i = 0; i < input.length; i++) {
    const char = input.charCodeAt(i)
    hash = ((hash << 5) - hash) + char
    hash = hash & hash // Convert to 32-bit integer
  }
  return Math.abs(hash).toString(36).slice(0, 8)
}

function generateBasicSourceMap(filePath: string, original: string, transformed: string): any {
  return {
    version: 3,
    sources: [filePath],
    names: [],
    mappings: 'AAAA', // Basic mapping
    file: filePath,
    sourcesContent: [original]
  }
}

// Implementation for remaining loaders (simplified)
async function applyPostCssLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const css = typeof content === 'string' ? content : content.toString('utf8')
  // PostCSS would be handled by the PostCSS WebWorker implementation
  return { content: css, meta: { postcss: 'delegated' } }
}

async function applyLessLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  return { content: typeof content === 'string' ? content : content.toString('utf8'), meta: { less: 'basic' } }
}

async function applyFileLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const filename = context.resourcePath.split('/').pop() || 'file'
  return { content: `module.exports = "data:application/octet-stream;base64,${Buffer.from(content).toString('base64')}";` }
}

async function applyUrlLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const options = context.getOptions()
  const limit = options.limit || 8192
  
  if (content.length < limit) {
    const mimeType = getMimeType(context.resourcePath)
    return { content: `module.exports = "data:${mimeType};base64,${Buffer.from(content).toString('base64')}";` }
  }
  
  return applyFileLoader(content, context)
}

async function applyRawLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const text = typeof content === 'string' ? content : content.toString('utf8')
  return { content: `module.exports = ${JSON.stringify(text)};` }
}

async function applyJsonLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const json = typeof content === 'string' ? content : content.toString('utf8')
  return { content: `module.exports = ${json};` }
}

async function applyHtmlLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const html = typeof content === 'string' ? content : content.toString('utf8')
  return { content: `module.exports = ${JSON.stringify(html)};` }
}

async function applyMarkdownLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const md = typeof content === 'string' ? content : content.toString('utf8')
  // Basic markdown to HTML
  const html = md
    .replace(/^# (.+)$/gm, '<h1>$1</h1>')
    .replace(/^## (.+)$/gm, '<h2>$1</h2>')
    .replace(/^### (.+)$/gm, '<h3>$1</h3>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
    .replace(/\n\n/g, '</p><p>')
  
  return { content: `module.exports = "<p>${html}</p>";` }
}

async function applySvgLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const svg = typeof content === 'string' ? content : content.toString('utf8')
  return { content: `module.exports = ${JSON.stringify(svg)};` }
}

async function applyWorkerLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  const source = typeof content === 'string' ? content : content.toString('utf8')
  const workerCode = `
const workerCode = ${JSON.stringify(source)};
const blob = new Blob([workerCode], { type: 'application/javascript' });
module.exports = function() { return new Worker(URL.createObjectURL(blob)); };
`
  return { content: workerCode }
}

async function applyEslintLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  // ESLint would emit warnings/errors but pass through content
  const source = typeof content === 'string' ? content : content.toString('utf8')
  context.emitWarning('ESLint checking simplified in WebWorker environment')
  return { content: source }
}

async function applySourceMapLoader(content: string | Buffer, context: LoaderContext): Promise<LoaderResult> {
  // Source map loader would process existing source maps
  const source = typeof content === 'string' ? content : content.toString('utf8')
  return { content: source, meta: { sourceMapProcessed: true } }
}

function getMimeType(filename: string): string {
  const ext = filename.split('.').pop()?.toLowerCase()
  const mimeTypes: Record<string, string> = {
    'png': 'image/png',
    'jpg': 'image/jpeg',
    'jpeg': 'image/jpeg',
    'gif': 'image/gif',
    'svg': 'image/svg+xml',
    'webp': 'image/webp',
    'css': 'text/css',
    'js': 'application/javascript',
    'json': 'application/json',
    'html': 'text/html',
    'txt': 'text/plain'
  }
  return mimeTypes[ext || ''] || 'application/octet-stream'
}

// Main transform function with enhanced capabilities
const transform = (
  ipc: TransformIpc,
  content: string | { binary: string },
  name: string,
  query: string,
  loaders: LoaderConfig[],
  sourceMap: boolean
) => {
  return new Promise<{ source: string; map?: string; assets?: any[]; warnings?: string[] }>((resolve, reject) => {
    const resource = name + query
    
    // Handle binary content
    let sourceContent: string | Buffer
    if (typeof content === 'string') {
      sourceContent = content
    } else {
      // For binary content, decode from base64
      sourceContent = Buffer.from(content.binary, 'base64')
    }
    
    runLoadersWebWorker(
      {
        resource,
        loaders: loaders.length > 0 ? loaders : [{ loader: 'identity-loader' }],
        readResource: (filename, callback) => {
          // In WebWorker environment, we already have the content
          callback(null, {
            toString: (encoding?: string) => {
              if (typeof sourceContent === 'string') {
                return sourceContent
              }
              return sourceContent.toString(encoding as BufferEncoding || 'utf8')
            }
          })
        },
      },
      (err, result) => {
        if (err) {
          reject(err)
          return
        }
        
        if (!result) {
          reject(new Error('No result from loader chain'))
          return
        }
        
        // Notify about dependencies
        ipc.sendInfo({
          type: 'dependencies',
          filePaths: result.dependencies || [],
          directories: result.contextDependencies || [],
          buildFilePaths: result.buildDependencies || [],
          envVariables: [],
        })
        
        const output = {
          source: typeof result.content === 'string' ? result.content : result.content.toString('utf8'),
          map: sourceMap && result.sourceMap ? JSON.stringify(result.sourceMap) : undefined,
          assets: result.meta?.assets,
          warnings: result.meta?.warnings
        }
        
        resolve(output)
      }
    )
  })
}

// Register basic loaders
function registerBasicLoaders() {
  // Raw loader - pass through content
  loaderRegistry.register('raw-loader', {
    name: 'raw-loader',
    version: '4.0.0-webworker',
    process: async (content, context) => {
      const text = typeof content === 'string' ? content : content.toString('utf8')
      return {
        content: `module.exports = ${JSON.stringify(text)};`,
        meta: { loader: 'raw-loader' }
      }
    }
  })

  // JSON loader
  loaderRegistry.register('json-loader', {
    name: 'json-loader', 
    version: '1.0.0-webworker',
    process: async (content, context) => {
      const text = typeof content === 'string' ? content : content.toString('utf8')
      try {
        JSON.parse(text) // Validate JSON
        return { content: `module.exports = ${text};`, meta: { loader: 'json-loader' } }
      } catch (e) {
        return { content: `module.exports = ${JSON.stringify(text)};`, meta: { loader: 'json-loader' } }
      }
    }
  })

  // File loader - convert to data URL
  loaderRegistry.register('file-loader', {
    name: 'file-loader',
    version: '6.0.0-webworker', 
    process: async (content, context) => {
      const resourcePath = context.resourcePath
      const filename = resourcePath.split('/').pop() || 'file'
      
      let base64Content: string
      if (typeof content === 'string') {
        base64Content = btoa(content)
      } else {
        base64Content = btoa(content.toString('binary'))
      }
      
      return {
        content: `module.exports = "data:application/octet-stream;base64,${base64Content}";`,
        meta: { loader: 'file-loader', filename }
      }
    }
  })

  // Basic CSS loader
  loaderRegistry.register('css-loader', {
    name: 'css-loader',
    version: '6.0.0-webworker',
    process: async (content, context) => {
      const css = typeof content === 'string' ? content : content.toString('utf8')
      
      // Check if it's a CSS module
      const isModule = context.resourcePath.includes('.module.') || 
                      context.resourcePath.includes('.modules.')
      
      if (isModule) {
        // Basic CSS Modules support
        const classNames: Record<string, string> = {}
        const processedCss = css.replace(/\.([a-zA-Z_][a-zA-Z0-9_-]*)/g, (match, className) => {
          const hashed = `${className}_${Math.random().toString(36).substr(2, 8)}`
          classNames[className] = hashed
          return `.${hashed}`
        })
        
        return {
          content: `module.exports = ${JSON.stringify(classNames)};\nmodule.exports._css = ${JSON.stringify(processedCss)};`,
          meta: { loader: 'css-loader', cssModules: classNames }
        }
      }
      
      return {
        content: `module.exports = ${JSON.stringify(css)};`,
        meta: { loader: 'css-loader' }
      }
    }
  })

  // Style loader
  loaderRegistry.register('style-loader', {
    name: 'style-loader',
    version: '3.0.0-webworker',
    process: async (content, context) => {
      const css = typeof content === 'string' ? content : content.toString('utf8')
      
      return {
        content: `
if (typeof document !== 'undefined') {
  const style = document.createElement('style');
  style.textContent = ${JSON.stringify(css)};
  document.head.appendChild(style);
}
module.exports = {};`,
        meta: { loader: 'style-loader' }
      }
    }
  })
}

export const init = async (ipc: TransformIpc) => {
  // Register basic loaders
  registerBasicLoaders()
  
  // Enhanced WebWorker initialization
  console.log('Enhanced WebWorker webpack loaders initialized with dynamic loader registry')
  console.log('Registered loaders:', loaderRegistry.getAvailable().join(', '))
  
  // Log the loader registry approach
  console.log('Loader resolution follows webpack conventions - supporting exact matches and base name matching')
}

export { transform } 