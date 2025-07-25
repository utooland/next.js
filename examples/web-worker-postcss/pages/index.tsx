import { useState, useEffect } from 'react'
import Head from 'next/head'
import Link from 'next/link'
import styles from '../styles/Home.module.css'
import { PostCssProcessor } from '../components/PostCssProcessor'

export default function Home() {
  const [isWorkerSupported, setIsWorkerSupported] = useState<boolean | null>(null)
  const [processingMode, setProcessingMode] = useState<'worker' | 'main-thread'>('worker')

  useEffect(() => {
    // 检测 Web Worker 支持
    setIsWorkerSupported(typeof Worker !== 'undefined')
  }, [])

  return (
    <div className={styles.container}>
      <Head>
        <title>Web Worker PostCSS Example</title>
        <meta name="description" content="Demonstrating PostCSS processing in Web Workers" />
        <link rel="icon" href="/favicon.ico" />
      </Head>

      <main className={styles.main}>
        <h1 className={styles.title}>
          Web Worker PostCSS Processing Example
        </h1>

        <div className={styles.description}>
          <p>
            This example demonstrates how PostCSS can be processed in Web Workers
            to avoid blocking the main thread.
          </p>
        </div>

        <div className={styles.navigation}>
          <Link href="/demo" className={styles.demoLink}>
            🚀 View Full Demo
          </Link>
        </div>

        <div className={styles.status}>
          <h2>Environment Status</h2>
          <div className={styles.statusGrid}>
            <div className={styles.statusItem}>
              <span className={styles.statusLabel}>Web Worker Support:</span>
              <span className={`${styles.statusValue} ${isWorkerSupported ? styles.supported : styles.notSupported}`}>
                {isWorkerSupported === null ? 'Checking...' : isWorkerSupported ? 'Supported' : 'Not Supported'}
              </span>
            </div>
            <div className={styles.statusItem}>
              <span className={styles.statusLabel}>Processing Mode:</span>
              <select 
                value={processingMode} 
                onChange={(e) => setProcessingMode(e.target.value as 'worker' | 'main-thread')}
                disabled={!isWorkerSupported}
                className={styles.select}
              >
                <option value="worker">Web Worker</option>
                <option value="main-thread">Main Thread</option>
              </select>
            </div>
          </div>
        </div>

        <PostCssProcessor mode={processingMode} />

        <div className={styles.features}>
          <h2>Features Demonstrated</h2>
          <ul className={styles.featureList}>
            <li>✅ PostCSS processing in Web Workers</li>
            <li>✅ Tailwind CSS support</li>
            <li>✅ Autoprefixer integration</li>
            <li>✅ Real-time CSS transformation</li>
            <li>✅ Performance monitoring</li>
            <li>✅ Fallback to main thread</li>
            <li>✅ Turbopack browser adapter</li>
            <li>✅ Loader runner in browser</li>
          </ul>
        </div>

        <div className={styles.architecture}>
          <h2>Architecture Overview</h2>
          <div className={styles.architectureGrid}>
            <div className={styles.archCard}>
              <h3>🔄 Loader Runner</h3>
              <p>Browser-compatible loader runner that processes modules through multiple loaders</p>
            </div>
            <div className={styles.archCard}>
              <h3>🎨 PostCSS Loader</h3>
              <p>PostCSS transformation with Tailwind CSS and Autoprefixer support</p>
            </div>
            <div className={styles.archCard}>
              <h3>👥 Web Worker Pool</h3>
              <p>Parallel processing using Web Workers to avoid blocking the main thread</p>
            </div>
            <div className={styles.archCard}>
              <h3>🔧 Turbopack Adapter</h3>
              <p>Browser adapter that makes Turbopack work in browser environments</p>
            </div>
          </div>
        </div>
      </main>

      <footer className={styles.footer}>
        <p>Built with Next.js and Turbopack</p>
      </footer>
    </div>
  )
} 