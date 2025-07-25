import { useState, useEffect, useRef } from 'react'
import styles from '../styles/PostCssProcessor.module.css'

interface PostCssProcessorProps {
  mode: 'worker' | 'main-thread'
}

interface ProcessingResult {
  css: string
  processingTime: number
  mode: string
  timestamp: number
  sourceMap?: string
  dependencies?: string[]
}

export function PostCssProcessor({ mode }: PostCssProcessorProps) {
  const [inputCss, setInputCss] = useState(`/* Example CSS with Tailwind and PostCSS features */
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer components {
  .btn-primary {
    @apply bg-primary-500 text-white px-4 py-2 rounded-lg hover:bg-primary-900 transition-colors;
  }
  
  .card {
    @apply bg-white shadow-lg rounded-lg p-6 border border-gray-200;
  }
}

/* Custom CSS with modern features */
.example-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1rem;
  padding: 1rem;
}

/* CSS with vendor prefixes that autoprefixer will handle */
.flexbox-example {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.transform-example {
  transform: translateX(10px) rotate(5deg);
  transition: transform 0.3s ease;
}

.transform-example:hover {
  transform: translateX(20px) rotate(10deg);
}`)

  const [result, setResult] = useState<ProcessingResult | null>(null)
  const [isProcessing, setIsProcessing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [turbopackAdapter, setTurbopackAdapter] = useState<any>(null)
  const workerRef = useRef<Worker | null>(null)

  // Initialize Turbopack adapter
  useEffect(() => {
    const initAdapter = async () => {
      try {
        // Load the Turbopack browser adapter
        const { TurbopackBrowserAdapter } = await import('../lib/turbopack-browser-adapter')
        
        const adapter = new TurbopackBrowserAdapter({
          useWorkers: mode === 'worker',
          workerPoolSize: 2
        })
        
        setTurbopackAdapter(adapter)
      } catch (error) {
        console.error('Failed to initialize Turbopack adapter:', error)
        setError('Failed to initialize Turbopack adapter')
      }
    }

    initAdapter()

    // Cleanup
    return () => {
      if (turbopackAdapter) {
        turbopackAdapter.cleanup()
      }
    }
  }, [mode])

  // Cleanup Web Worker
  useEffect(() => {
    return () => {
      if (workerRef.current) {
        workerRef.current.terminate()
      }
    }
  }, [])

  const processCss = async () => {
    if (!turbopackAdapter) {
      setError('Turbopack adapter not initialized')
      return
    }

    setIsProcessing(true)
    setError(null)
    const startTime = performance.now()

    try {
      let processedResult: any
      let processingMode: string

      if (mode === 'worker') {
        // Use Turbopack adapter with Web Workers
        processingMode = 'Turbopack Web Worker'
        processedResult = await turbopackAdapter.processCssFile('example.css', inputCss)
      } else {
        // Use Turbopack adapter in main thread
        processingMode = 'Turbopack Main Thread'
        processedResult = await turbopackAdapter.processCssFile('example.css', inputCss)
      }

      const endTime = performance.now()
      const processingTime = endTime - startTime

      setResult({
        css: processedResult.source,
        processingTime,
        mode: processingMode,
        timestamp: Date.now(),
        sourceMap: processedResult.sourceMap,
        dependencies: processedResult.dependencies
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error occurred')
    } finally {
      setIsProcessing(false)
    }
  }

  const processCssWithLegacyWorker = async () => {
    setIsProcessing(true)
    setError(null)
    const startTime = performance.now()

    try {
      let processedCss: string
      let processingMode: string

      if (mode === 'worker' && typeof Worker !== 'undefined') {
        // Use legacy Web Worker processing
        processingMode = 'Legacy Web Worker'
        processedCss = await processCssInWorker(inputCss)
      } else {
        // Use legacy main thread processing
        processingMode = 'Legacy Main Thread'
        processedCss = await processCssInMainThread(inputCss)
      }

      const endTime = performance.now()
      const processingTime = endTime - startTime

      setResult({
        css: processedCss,
        processingTime,
        mode: processingMode,
        timestamp: Date.now()
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error occurred')
    } finally {
      setIsProcessing(false)
    }
  }

  const processCssInWorker = (css: string): Promise<string> => {
    return new Promise((resolve, reject) => {
      // Create Web Worker
      const worker = new Worker('/postcss-worker.js')
      workerRef.current = worker

      const timeout = setTimeout(() => {
        worker.terminate()
        reject(new Error('Worker processing timeout'))
      }, 10000) // 10秒超时

      worker.onmessage = (event) => {
        clearTimeout(timeout)
        const { type, data, error } = event.data

        if (type === 'result') {
          resolve(data.css)
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

      // Send processing request
      worker.postMessage({
        type: 'process',
        css,
        config: {
          plugins: {
            'tailwindcss': {},
            'autoprefixer': {
              overrideBrowserslist: ['> 1%', 'last 2 versions', 'not dead']
            }
          }
        },
        sourceMap: false
      })
    })
  }

  const processCssInMainThread = async (css: string): Promise<string> => {
    // 模拟主线程处理
    // 在实际实现中，这里会调用 PostCSS 的同步 API
    await new Promise(resolve => setTimeout(resolve, 100)) // 模拟处理时间
    
    // 简化的处理逻辑
    let processedCss = css
    
    // 模拟 Tailwind CSS 处理
    processedCss = processedCss.replace(/@tailwind\s+(\w+);/g, '/* Tailwind $1 styles */')
    
    // 模拟 Autoprefixer 处理
    processedCss = processedCss.replace(/display:\s*flex/g, 'display: -webkit-flex; display: -ms-flexbox; display: flex')
    processedCss = processedCss.replace(/transform:\s*([^;]+);/g, 'transform: $1; -webkit-transform: $1; -ms-transform: $1;')
    
    return processedCss
  }

  return (
    <div className={styles.processor}>
      <h2>PostCSS Processor with Turbopack Browser Adapter</h2>
      
      <div className={styles.inputSection}>
        <h3>Input CSS</h3>
        <textarea
          value={inputCss}
          onChange={(e) => setInputCss(e.target.value)}
          className={styles.cssInput}
          placeholder="Enter your CSS here..."
          rows={15}
        />
      </div>

      <div className={styles.controls}>
        <div className={styles.buttonGroup}>
          <button
            onClick={processCss}
            disabled={isProcessing || !turbopackAdapter}
            className={styles.processButton}
          >
            {isProcessing ? 'Processing...' : 'Process with Turbopack'}
          </button>
          
          <button
            onClick={processCssWithLegacyWorker}
            disabled={isProcessing}
            className={`${styles.processButton} ${styles.secondaryButton}`}
          >
            {isProcessing ? 'Processing...' : 'Process with Legacy'}
          </button>
        </div>
        
        <div className={styles.modeInfo}>
          Processing Mode: <strong>{mode === 'worker' ? 'Web Worker' : 'Main Thread'}</strong>
          {turbopackAdapter && (
            <span className={styles.adapterStatus}>
              | Turbopack Adapter: <span className={styles.ready}>Ready</span>
            </span>
          )}
        </div>
      </div>

      {error && (
        <div className={styles.error}>
          <h3>Error</h3>
          <p>{error}</p>
        </div>
      )}

      {result && (
        <div className={styles.resultSection}>
          <h3>Processed CSS</h3>
          <div className={styles.resultInfo}>
            <span>Processing Time: <strong>{result.processingTime.toFixed(2)}ms</strong></span>
            <span>Mode: <strong>{result.mode}</strong></span>
            <span>Timestamp: <strong>{new Date(result.timestamp).toLocaleTimeString()}</strong></span>
            {result.dependencies && result.dependencies.length > 0 && (
              <span>Dependencies: <strong>{result.dependencies.length}</strong></span>
            )}
          </div>
          <pre className={styles.cssOutput}>
            <code>{result.css}</code>
          </pre>
          
          {result.sourceMap && (
            <details className={styles.sourceMapSection}>
              <summary>Source Map</summary>
              <pre className={styles.sourceMapOutput}>
                <code>{result.sourceMap}</code>
              </pre>
            </details>
          )}
          
          <div className={styles.preview}>
            <h4>Live Preview</h4>
            <div 
              className={styles.previewContainer}
              dangerouslySetInnerHTML={{
                __html: `
                  <style>${result.css}</style>
                  <div class="card">
                    <h3>Sample Card</h3>
                    <p>This is a sample card with processed CSS.</p>
                    <button class="btn-primary">Click me</button>
                  </div>
                  <div class="example-grid">
                    <div class="card">Grid Item 1</div>
                    <div class="card">Grid Item 2</div>
                    <div class="card">Grid Item 3</div>
                  </div>
                  <div class="flexbox-example">
                    <div class="transform-example">Hover me!</div>
                  </div>
                `
              }}
            />
          </div>
        </div>
      )}
    </div>
  )
} 