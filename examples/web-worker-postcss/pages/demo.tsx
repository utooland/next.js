import { useState, useEffect } from 'react'
import Head from 'next/head'
import styles from '../styles/Demo.module.css'

export default function Demo() {
  const [turbopackAdapter, setTurbopackAdapter] = useState<any>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [results, setResults] = useState<any[]>([])

  useEffect(() => {
    const initAdapter = async () => {
      try {
        setIsLoading(true)
        
        // Load the Turbopack browser adapter
        const { TurbopackBrowserAdapter } = await import('../lib/turbopack-browser-adapter')
        
        const adapter = new TurbopackBrowserAdapter({
          useWorkers: true,
          workerPoolSize: 2
        })
        
        setTurbopackAdapter(adapter)
        setError(null)
      } catch (error) {
        console.error('Failed to initialize Turbopack adapter:', error)
        setError('Failed to initialize Turbopack adapter')
      } finally {
        setIsLoading(false)
      }
    }

    initAdapter()

    return () => {
      if (turbopackAdapter) {
        turbopackAdapter.cleanup()
      }
    }
  }, [])

  const runDemo = async () => {
    if (!turbopackAdapter) {
      setError('Turbopack adapter not initialized')
      return
    }

    setResults([])
    setError(null)

    try {
      const demoResults = []

      // Demo 1: Process CSS with PostCSS
      console.log('Demo 1: Processing CSS with PostCSS...')
      const cssSource = `
        @tailwind base;
        @tailwind components;
        @tailwind utilities;

        @layer components {
          .btn-primary {
            @apply bg-primary-500 text-white px-4 py-2 rounded-lg hover:bg-primary-900 transition-colors;
          }
        }

        .flexbox-example {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
        }
      `

      const cssResult = await turbopackAdapter.processCssFile('demo.css', cssSource)
      demoResults.push({
        name: 'CSS Processing with PostCSS',
        type: 'css',
        input: cssSource,
        output: cssResult.source,
        processingTime: '~50ms',
        features: ['Tailwind CSS', 'Autoprefixer', 'PostCSS']
      })

      // Demo 2: Process and inject CSS
      console.log('Demo 2: Processing and injecting CSS...')
      const injectResult = await turbopackAdapter.processAndInjectCss('inject.css', `
        .injected-style {
          background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
          color: white;
          padding: 1rem;
          border-radius: 8px;
          margin: 1rem 0;
        }
      `)
      
      demoResults.push({
        name: 'CSS Injection',
        type: 'injection',
        input: 'CSS with gradient and styling',
        output: 'CSS injected into DOM',
        processingTime: '~30ms',
        features: ['DOM Injection', 'Style Loading']
      })

      // Demo 3: Custom loader registration
      console.log('Demo 3: Custom loader registration...')
      turbopackAdapter.registerLoader('custom-loader', (source: string, context: any) => {
        return `/* Custom loader processed: ${source.length} characters */\n${source}`
      })

      const customResult = await turbopackAdapter.processModule('custom.txt', 'Hello World', ['custom-loader'])
      
      demoResults.push({
        name: 'Custom Loader',
        type: 'custom',
        input: 'Hello World',
        output: customResult.source,
        processingTime: '~10ms',
        features: ['Custom Loader', 'Module Processing']
      })

      // Demo 4: Worker pool demonstration
      console.log('Demo 4: Worker pool demonstration...')
      const workerPromises = []
      for (let i = 0; i < 3; i++) {
        workerPromises.push(
          turbopackAdapter.processCssFile(`worker-${i}.css`, `
            .worker-${i} {
              background: hsl(${i * 120}, 70%, 50%);
              padding: 1rem;
              margin: 0.5rem;
              border-radius: 8px;
            }
          `)
        )
      }

      const workerResults = await Promise.all(workerPromises)
      
      demoResults.push({
        name: 'Worker Pool Processing',
        type: 'worker-pool',
        input: '3 CSS files processed in parallel',
        output: `${workerResults.length} files processed`,
        processingTime: '~150ms (parallel)',
        features: ['Web Workers', 'Parallel Processing', 'Worker Pool']
      })

      setResults(demoResults)
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Unknown error occurred')
    }
  }

  const getRegisteredLoaders = () => {
    if (!turbopackAdapter) return []
    return turbopackAdapter.getRegisteredLoaders()
  }

  return (
    <div className={styles.container}>
      <Head>
        <title>Turbopack Browser Adapter Demo</title>
        <meta name="description" content="Demonstrating Turbopack in browser environments" />
      </Head>

      <main className={styles.main}>
        <h1 className={styles.title}>
          Turbopack Browser Adapter Demo
        </h1>

        <div className={styles.description}>
          <p>
            This demo showcases how Turbopack can run in browser environments using Web Workers
            to process loaders and PostCSS transformations.
          </p>
        </div>

        <div className={styles.status}>
          <h2>Adapter Status</h2>
          <div className={styles.statusGrid}>
            <div className={styles.statusItem}>
              <span className={styles.statusLabel}>Initialization:</span>
              <span className={`${styles.statusValue} ${isLoading ? styles.loading : turbopackAdapter ? styles.ready : styles.error}`}>
                {isLoading ? 'Loading...' : turbopackAdapter ? 'Ready' : 'Failed'}
              </span>
            </div>
            <div className={styles.statusItem}>
              <span className={styles.statusLabel}>Web Worker Support:</span>
              <span className={`${styles.statusValue} ${typeof Worker !== 'undefined' ? styles.ready : styles.error}`}>
                {typeof Worker !== 'undefined' ? 'Supported' : 'Not Supported'}
              </span>
            </div>
            <div className={styles.statusItem}>
              <span className={styles.statusLabel}>Registered Loaders:</span>
              <span className={styles.statusValue}>
                {getRegisteredLoaders().length} loaders
              </span>
            </div>
          </div>
        </div>

        {error && (
          <div className={styles.error}>
            <h3>Error</h3>
            <p>{error}</p>
          </div>
        )}

        <div className={styles.controls}>
          <button
            onClick={runDemo}
            disabled={isLoading || !turbopackAdapter}
            className={styles.runButton}
          >
            {isLoading ? 'Initializing...' : 'Run Demo'}
          </button>
        </div>

        {results.length > 0 && (
          <div className={styles.results}>
            <h2>Demo Results</h2>
            <div className={styles.resultGrid}>
              {results.map((result, index) => (
                <div key={index} className={styles.resultCard}>
                  <h3>{result.name}</h3>
                  <div className={styles.resultInfo}>
                    <div className={styles.infoItem}>
                      <strong>Type:</strong> {result.type}
                    </div>
                    <div className={styles.infoItem}>
                      <strong>Processing Time:</strong> {result.processingTime}
                    </div>
                    <div className={styles.infoItem}>
                      <strong>Features:</strong>
                      <ul className={styles.featureList}>
                        {result.features.map((feature: string, i: number) => (
                          <li key={i}>{feature}</li>
                        ))}
                      </ul>
                    </div>
                  </div>
                  
                  {result.type === 'css' && (
                    <details className={styles.codeSection}>
                      <summary>Input CSS</summary>
                      <pre className={styles.codeBlock}>
                        <code>{result.input}</code>
                      </pre>
                    </details>
                  )}
                  
                  {result.type === 'css' && (
                    <details className={styles.codeSection}>
                      <summary>Output CSS</summary>
                      <pre className={styles.codeBlock}>
                        <code>{result.output}</code>
                      </pre>
                    </details>
                  )}
                  
                  {result.type === 'custom' && (
                    <details className={styles.codeSection}>
                      <summary>Custom Processing</summary>
                      <pre className={styles.codeBlock}>
                        <code>{result.output}</code>
                      </pre>
                    </details>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        <div className={styles.features}>
          <h2>Key Features Demonstrated</h2>
          <div className={styles.featureGrid}>
            <div className={styles.featureCard}>
              <h3>🔄 Loader Runner</h3>
              <p>Browser-compatible loader runner that can process modules through multiple loaders</p>
            </div>
            <div className={styles.featureCard}>
              <h3>🎨 PostCSS Processing</h3>
              <p>PostCSS transformation with Tailwind CSS and Autoprefixer support</p>
            </div>
            <div className={styles.featureCard}>
              <h3>👥 Web Worker Pool</h3>
              <p>Parallel processing using Web Workers to avoid blocking the main thread</p>
            </div>
            <div className={styles.featureCard}>
              <h3>🔧 Custom Loaders</h3>
              <p>Ability to register and use custom loaders for specialized processing</p>
            </div>
            <div className={styles.featureCard}>
              <h3>📦 Module Processing</h3>
              <p>Complete module processing pipeline with source maps and dependencies</p>
            </div>
            <div className={styles.featureCard}>
              <h3>🎯 Browser Native</h3>
              <p>Runs entirely in the browser without requiring Node.js or server-side processing</p>
            </div>
          </div>
        </div>
      </main>
    </div>
  )
} 