// Web Worker 中的 PostCSS 处理器
// 这个文件将在 Web Worker 中运行，用于处理 PostCSS 转换

// 导入 PostCSS 和相关插件
// 注意：在 Web Worker 中，我们需要使用支持 Web Worker 的 PostCSS 版本

let postcss = null
let autoprefixer = null
let tailwindcss = null

// 动态导入 PostCSS 和相关插件
async function loadPostCss() {
  if (!postcss) {
    try {
      // 在 Web Worker 中加载 PostCSS
      // 这里我们需要确保 PostCSS 和相关插件支持 Web Worker 环境
      const postcssModule = await import('postcss')
      postcss = postcssModule.default || postcssModule
      
      // 加载常用的插件
      try {
        const autoprefixerModule = await import('autoprefixer')
        autoprefixer = autoprefixerModule.default || autoprefixerModule
      } catch (e) {
        console.warn('Autoprefixer not available in Web Worker')
      }
      
      try {
        const tailwindModule = await import('tailwindcss')
        tailwindcss = tailwindModule.default || tailwindModule
      } catch (e) {
        console.warn('Tailwind CSS not available in Web Worker')
      }
    } catch (error) {
      console.error('Failed to load PostCSS in Web Worker:', error)
      throw error
    }
  }
}

// 处理 PostCSS 转换
async function processPostCss(css, config, sourceMap) {
  await loadPostCss()
  
  if (!postcss) {
    throw new Error('PostCSS not available in Web Worker')
  }
  
  // 构建插件列表
  const plugins = []
  
  if (config.plugins) {
    for (const plugin of config.plugins) {
      const [pluginName, pluginOptions] = Array.isArray(plugin) ? plugin : [plugin, {}]
      
      switch (pluginName) {
        case 'autoprefixer':
          if (autoprefixer) {
            plugins.push(autoprefixer(pluginOptions))
          }
          break
        case 'tailwindcss':
          if (tailwindcss) {
            plugins.push(tailwindcss(pluginOptions))
          }
          break
        default:
          // 对于其他插件，我们可以尝试动态加载
          try {
            const pluginModule = await import(pluginName)
            const pluginFn = pluginModule.default || pluginModule
            plugins.push(pluginFn(pluginOptions))
          } catch (e) {
            console.warn(`Plugin ${pluginName} not available in Web Worker`)
          }
      }
    }
  }
  
  // 执行 PostCSS 处理
  const result = await postcss(plugins).process(css, {
    from: undefined,
    to: undefined,
    map: sourceMap ? { inline: false } : false
  })
  
  return {
    css: result.css,
    map: result.map ? result.map.toString() : undefined,
    assets: []
  }
}

// 监听主线程消息
self.addEventListener('message', async (event) => {
  const { type, css, config, sourceMap } = event.data
  
  if (type === 'process') {
    try {
      const result = await processPostCss(css, config, sourceMap)
      self.postMessage({
        type: 'result',
        data: result
      })
    } catch (error) {
      self.postMessage({
        type: 'error',
        error: error.message
      })
    }
  }
})

// 错误处理
self.addEventListener('error', (event) => {
  console.error('Web Worker error:', event.error)
  self.postMessage({
    type: 'error',
    error: event.error.message
  })
})

// 未处理的 Promise 拒绝
self.addEventListener('unhandledrejection', (event) => {
  console.error('Unhandled promise rejection in Web Worker:', event.reason)
  self.postMessage({
    type: 'error',
    error: event.reason.message || 'Unhandled promise rejection'
  })
}) 