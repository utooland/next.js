// This is the compiled version of webpack-loaders-webworker.ts for WebWorker execution
// DO NOT EDIT - This file is auto-generated from webpack-loaders-webworker.ts

const { runLoaders } = require('@vercel/turbopack/loader-runner');

// Buffer polyfill for WebWorker
if (typeof Buffer === 'undefined') {
  global.Buffer = require('buffer').Buffer;
}

class WebWorkerIpc {
  async invoke(messageType, ...args) {
    if (messageType === 'transform') {
      return await transform(...args);
    }
    throw new Error(`Unknown message type: ${messageType}`);
  }
}

const ipc = new WebWorkerIpc();

// Main message handler for WebWorker
self.onmessage = async function(event) {
  try {
    const { messageType, id, payload } = event.data;
    const result = await ipc.invoke(messageType, ...payload);
    self.postMessage({ id, result });
  } catch (error) {
    self.postMessage({ 
      id: event.data.id, 
      error: {
        name: error.name || 'Error',
        message: error.message || 'Unknown error',
        stack: error.stack || ''
      }
    });
  }
};

// Transform function that processes webpack loaders
async function transform(content, name, query, loaders, sourceMap, cwd) {
  return new Promise((resolve, reject) => {
    const resource = name + (query || '');
    
    // Handle binary content
    let processedContent;
    if (typeof content === 'object' && content.binary) {
      processedContent = Buffer.from(content.binary, 'base64');
    } else {
      processedContent = content;
    }

    // Create loader context
    const loaderContext = {
      version: 2,
      resource,
      resourcePath: name,
      resourceQuery: query || '',
      async: () => {
        let isAsync = false;
        const callback = (err, content, map, meta) => {
          if (isAsync) {
            if (err) {
              reject(err);
            } else {
              resolve({
                source: content || '',
                map: map ? (typeof map === 'string' ? map : JSON.stringify(map)) : null,
                assets: null,
                warnings: null,
                errors: null
              });
            }
          }
        };
        isAsync = true;
        return callback;
      },
      callback: null,
      cacheable: () => {},
      addDependency: () => {},
      addContextDependency: () => {},
      clearDependencies: () => {},
      emitWarning: () => {},
      emitError: () => {},
      emitFile: () => {},
      fs: null,
      utils: null,
      query: query || '',
      data: {},
      getOptions: () => ({}),
      resolve: () => {},
      getResolve: () => () => {},
      environment: { arrowFunction: true, bigIntLiteral: true, const: true, destructuring: true, dynamicImport: true, forOf: true, module: true },
      target: 'web',
      webpack: true,
      sourceMap: sourceMap,
      mode: 'development',
      hot: false,
      minimize: false,
      _module: null,
      _compilation: null,
      rootContext: cwd || process.cwd()
    };

    // Run loaders
    runLoaders({
      resource,
      loaders: loaders.map(loader => ({
        loader: loader.loader,
        options: loader.options || {}
      })),
      context: loaderContext,
      processResource: (loaderContext, resource, callback) => {
        callback(null, processedContent, null);
      }
    }, (err, result) => {
      if (err) {
        reject(err);
        return;
      }

      const source = result && result.result && result.result[0] 
        ? (typeof result.result[0] === 'object' && result.result[0].binary
           ? result.result[0].binary
           : result.result[0] || '')
        : '';
      
      const map = result && result.result && result.result[1]
        ? (typeof result.result[1] === 'string'
           ? JSON.parse(result.result[1])
           : result.result[1])
        : null;

      resolve({
        source,
        map: map ? JSON.stringify(map) : null,
        assets: null,
        warnings: null,
        errors: null
      });
    });
  });
} 