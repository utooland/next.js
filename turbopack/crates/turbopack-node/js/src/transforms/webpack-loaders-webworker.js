(() => { // webpackBootstrap
  "use strict";
  var __webpack_modules__ = ({
      91: (function () {

          class LoaderLoadingError extends Error {
              constructor(message) {
                  super(message);
                  this.name = "LoaderLoadingError";
              }
          }
          // 为 Web Worker 环境提供全局访问
          if (typeof self !== 'undefined') {
              self.LoaderLoadingError = LoaderLoadingError;
          }


      }),
      257: (function () {
          /*
              MIT License http://www.opensource.org/licenses/mit-license.php
              Author Tobias Koppers @sokra
          */
          const Buffer = typeof globalThis !== 'undefined' && globalThis.Buffer || typeof self !== 'undefined' && self.Buffer || typeof window !== 'undefined' && window.Buffer || null; // 移除对 buffer 模块的依赖
          // Web Worker 环境下的文件读取函数
          function readFile(path, callback) {
              // 在 Web Worker 中，文件读取需要通过 postMessage 与主线程通信
              // 或者使用预加载的文件内容
              if (self.__fileContents__ && self.__fileContents__[path]) {
                  return callback(null, self.__fileContents__[path]);
              }
              // 如果没有预加载的文件内容，尝试通过 postMessage 请求
              if (self.postMessage) {
                  const requestId = Math.random().toString(36).substr(2, 9);
                  // 设置响应处理器
                  const responseHandler = (event) => {
                      if (event.data.type === 'fileResponse' && event.data.requestId === requestId) {
                          self.removeEventListener('message', responseHandler);
                          if (event.data.error) {
                              callback(new Error(event.data.error));
                          } else {
                              callback(null, event.data.content);
                          }
                      }
                  };
                  self.addEventListener('message', responseHandler);
                  // 发送文件读取请求
                  self.postMessage({
                      type: 'readFile',
                      requestId,
                      path
                  });
                  // 设置超时
                  setTimeout(() => {
                      self.removeEventListener('message', responseHandler);
                      callback(new Error(`File read timeout for: ${path}`));
                  }, 10000);
              } else {
                  callback(new Error(`Cannot read file in Web Worker: ${path}`));
              }
          }
          function utf8BufferToString(buf) {
              const str = buf.toString("utf8");
              if (str.charCodeAt(0) === 0xfeff) {
                  return str.slice(1);
              }
              return str;
          }
          const PATH_QUERY_FRAGMENT_REGEXP = /^((?:\0.|[^?#\0])*)(\?(?:\0.|[^#\0])*)?(#.*)?$/;
  /**
   * @param {string} str the path with query and fragment
   * @returns {{ path: string, query: string, fragment: string }} parsed parts
   */ function parsePathQueryFragment(str) {
              const match = PATH_QUERY_FRAGMENT_REGEXP.exec(str);
              return {
                  path: match[1].replace(/\0(.)/g, "$1"),
                  query: match[2] ? match[2].replace(/\0(.)/g, "$1") : "",
                  fragment: match[3] || ""
              };
          }
          function dirname(path) {
              if (path === "/") return "/";
              const i = path.lastIndexOf("/");
              const j = path.lastIndexOf("\\");
              const i2 = path.indexOf("/");
              const j2 = path.indexOf("\\");
              const idx = i > j ? i : j;
              const idx2 = i > j ? i2 : j2;
              if (idx < 0) return path;
              if (idx === idx2) return path.slice(0, idx + 1);
              return path.slice(0, idx);
          }
          function createLoaderObject(loader) {
              const obj = {
                  path: null,
                  query: null,
                  fragment: null,
                  options: null,
                  ident: null,
                  normal: null,
                  pitch: null,
                  raw: null,
                  data: null,
                  pitchExecuted: false,
                  normalExecuted: false
              };
              Object.defineProperty(obj, "request", {
                  enumerable: true,
                  get() {
                      return obj.path.replace(/#/g, "\0#") + obj.query.replace(/#/g, "\0#") + obj.fragment;
                  },
                  set(value) {
                      if (typeof value === "string") {
                          const splittedRequest = parsePathQueryFragment(value);
                          obj.path = splittedRequest.path;
                          obj.query = splittedRequest.query;
                          obj.fragment = splittedRequest.fragment;
                          obj.options = undefined;
                          obj.ident = undefined;
                      } else {
                          if (!value.loader) {
                              throw new Error(`request should be a string or object with loader and options (${JSON.stringify(value)})`);
                          }
                          obj.path = value.loader;
                          obj.fragment = value.fragment || "";
                          obj.type = value.type;
                          obj.options = value.options;
                          obj.ident = value.ident;
                          // 修复：确保 query 字段被正确设置
                          if (obj.options === null) {
                              obj.query = "";
                          } else if (obj.options === undefined) {
                              obj.query = "";
                          } else if (typeof obj.options === "string") {
                              obj.query = `?${obj.options}`;
                          } else if (obj.ident) {
                              obj.query = `??${obj.ident}`;
                          } else if (typeof obj.options === "object" && obj.options.ident) {
                              obj.query = `??${obj.options.ident}`;
                          } else {
                              obj.query = `?${JSON.stringify(obj.options)}`;
                          }
                          // 添加调试信息
                          if (typeof self !== 'undefined' && self.postMessage) {
                              self.postMessage({
                                  type: 'debug',
                                  message: `\u{1F527} createLoaderObject: path=${obj.path}, query=${obj.query}, options=${JSON.stringify(obj.options)}`
                              });
                          }
                      }
                  }
              });
              obj.request = loader;
              if (Object.preventExtensions) {
                  Object.preventExtensions(obj);
              }
              return obj;
          }
          function runSyncOrAsync(fn, context, args, callback) {
              let isSync = true;
              let isDone = false;
              let isError = false; // internal error
              let reportedError = false;
              // eslint-disable-next-line func-name-matching
              const innerCallback = context.callback = function innerCallback() {
                  if (isDone) {
                      if (reportedError) return; // ignore
                      throw new Error("callback(): The callback was already called.");
                  }
                  isDone = true;
                  isSync = false;
                  try {
                      callback.apply(null, arguments);
                  } catch (err) {
                      isError = true;
                      throw err;
                  }
              };
              context.async = function async() {
                  if (isDone) {
                      if (reportedError) return; // ignore
                      throw new Error("async(): The callback was already called.");
                  }
                  isSync = false;
                  return innerCallback;
              };
              try {
                  const result = function LOADER_EXECUTION() {
                      return fn.apply(context, args);
                  }();
                  if (isSync) {
                      isDone = true;
                      if (result === undefined) return callback();
                      if (result && typeof result === "object" && typeof result.then === "function") {
                          return result.then((r) => {
                              callback(null, r);
                          }, callback);
                      }
                      return callback(null, result);
                  }
              } catch (err) {
                  if (isError) throw err;
                  if (isDone) {
                      // loader is already "done", so we cannot use the callback function
                      // for better debugging we print the error on the console
                      if (typeof err === "object" && err.stack) {
                          // eslint-disable-next-line no-console
                          console.error(err.stack);
                      } else {
                          // eslint-disable-next-line no-console
                          console.error(err);
                      }
                      return;
                  }
                  isDone = true;
                  reportedError = true;
                  callback(err);
              }
          }
          function convertArgs(args, raw) {
              if (!raw && Buffer && Buffer.isBuffer(args[0])) {
                  args[0] = utf8BufferToString(args[0]);
              } else if (raw && typeof args[0] === "string") {
                  args[0] = Buffer.from(args[0], "utf8");
              }
          }
          function iterateNormalLoaders(options, loaderContext, args, callback) {
              // 添加调试信息
              if (typeof self !== 'undefined' && self.postMessage) {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} iterateNormalLoaders \u{88AB}\u{8C03}\u{7528}, loaderIndex=${loaderContext.loaderIndex}, loaders.length=${loaderContext.loaders.length}`
                  });
              }
              if (loaderContext.loaderIndex < 0) return callback(null, args);
              const currentLoaderObject = loaderContext.loaders[loaderContext.loaderIndex];
              // 添加调试信息
              if (typeof self !== 'undefined' && self.postMessage) {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} \u{5F53}\u{524D} loader: ${currentLoaderObject.path}, normalExecuted=${currentLoaderObject.normalExecuted}`
                  });
              }
              // iterate
              if (currentLoaderObject.normalExecuted) {
                  loaderContext.loaderIndex--;
                  return iterateNormalLoaders(options, loaderContext, args, callback);
              }
              const fn = currentLoaderObject.normal;
              currentLoaderObject.normalExecuted = true;
              if (!fn) return iterateNormalLoaders(options, loaderContext, args, callback);
              // 添加调试信息
              if (typeof self !== 'undefined' && self.postMessage) {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} \u{51C6}\u{5907}\u{6267}\u{884C} loader function, args[0] \u{957F}\u{5EA6}: ${args[0] ? args[0].length : 'undefined'}`
                  });
              }
              convertArgs(args, currentLoaderObject.raw);
              runSyncOrAsync(fn, loaderContext, args, function runSyncOrAsyncCallback(err) {
                  // 添加调试信息
                  if (typeof self !== 'undefined' && self.postMessage) {
                      self.postMessage({
                          type: 'debug',
                          message: `\u{1F527} loader \u{6267}\u{884C}\u{5B8C}\u{6210}, err=${err ? 'yes' : 'no'}, \u{7ED3}\u{679C}\u{53C2}\u{6570}\u{6570}\u{91CF}: ${arguments.length}`
                      });
                  }
                  if (err) return callback(err);
                  const args = Array.prototype.slice.call(arguments, 1);
                  iterateNormalLoaders(options, loaderContext, args, callback);
              });
          }
          function processResource(options, loaderContext, callback) {
              // 添加调试信息
              if (typeof self !== 'undefined' && self.postMessage) {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} processResource \u{88AB}\u{8C03}\u{7528}\u{FF0C}resource: ${loaderContext.resource}`
                  });
              }
              const resource = loaderContext.resource;
              if (!resource) return callback(null, []);
              options.processResource(options.readResource || readFile, loaderContext, resource, function processResourceCallback(err, buffer) {
                  // 添加调试信息
                  if (typeof self !== 'undefined' && self.postMessage) {
                      self.postMessage({
                          type: 'debug',
                          message: `\u{1F527} processResource \u{56DE}\u{8C03}\u{FF0C}err=${err ? 'yes' : 'no'}\u{FF0C}buffer\u{957F}\u{5EA6}: ${buffer ? buffer.length : 'undefined'}`
                      });
                  }
                  if (err) return callback(err);
                  options.resourceBuffer = buffer;
                  // 设置 loaderIndex 为最后一个 loader，准备从后往前执行 normal loaders
                  loaderContext.loaderIndex = loaderContext.loaders.length - 1;
                  // 添加调试信息
                  if (typeof self !== 'undefined' && self.postMessage) {
                      self.postMessage({
                          type: 'debug',
                          message: `\u{1F527} processResource \u{5B8C}\u{6210}\u{FF0C}\u{5F00}\u{59CB}\u{6267}\u{884C} normal loaders\u{FF0C}loaderIndex=${loaderContext.loaderIndex}`
                      });
                  }
                  // 调用 iterateNormalLoaders 来执行所有 normal loader 函数
                  iterateNormalLoaders(options, loaderContext, [
                      buffer
                  ], callback);
              });
          }
          function iteratePitchingLoaders(options, loaderContext, callback) {
              // 添加调试信息
              if (typeof self !== 'undefined' && self.postMessage) {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} iteratePitchingLoaders \u{88AB}\u{8C03}\u{7528}\u{FF0C}loaderIndex=${loaderContext.loaderIndex}\u{FF0C}loaders.length=${loaderContext.loaders.length}`
                  });
              }
              // abort after last loader
              if (loaderContext.loaderIndex >= loaderContext.loaders.length) {
                  if (typeof self !== 'undefined' && self.postMessage) {
                      self.postMessage({
                          type: 'debug',
                          message: `\u{1F527} Pitching \u{9636}\u{6BB5}\u{5B8C}\u{6210}\u{FF0C}\u{5F00}\u{59CB} processResource`
                      });
                  }
                  return processResource(options, loaderContext, callback);
              }
              const currentLoaderObject = loaderContext.loaders[loaderContext.loaderIndex];
              // 添加调试信息
              if (typeof self !== 'undefined' && self.postMessage) {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} \u{5F53}\u{524D} pitching loader: ${currentLoaderObject.path}\u{FF0C}pitchExecuted=${currentLoaderObject.pitchExecuted}`
                  });
              }
              // iterate
              if (currentLoaderObject.pitchExecuted) {
                  loaderContext.loaderIndex++;
                  return iteratePitchingLoaders(options, loaderContext, callback);
              }
              // load loader module
              loadLoader(currentLoaderObject, (err) => {
                  if (err) {
                      loaderContext.cacheable(false);
                      return callback(err);
                  }
                  const fn = currentLoaderObject.pitch;
                  currentLoaderObject.pitchExecuted = true;
                  if (!fn) {
                      if (typeof self !== 'undefined' && self.postMessage) {
                          self.postMessage({
                              type: 'debug',
                              message: `\u{1F527} \u{6CA1}\u{6709} pitch \u{51FD}\u{6570}\u{FF0C}\u{7EE7}\u{7EED}\u{4E0B}\u{4E00}\u{4E2A} loader`
                          });
                      }
                      return iteratePitchingLoaders(options, loaderContext, callback);
                  }
                  if (typeof self !== 'undefined' && self.postMessage) {
                      self.postMessage({
                          type: 'debug',
                          message: `\u{1F527} \u{6267}\u{884C} pitch \u{51FD}\u{6570}`
                      });
                  }
                  runSyncOrAsync(fn, loaderContext, [
                      loaderContext.remainingRequest,
                      loaderContext.previousRequest,
                      currentLoaderObject.data = {}
                  ], function runSyncOrAsyncCallback(err) {
                      if (err) return callback(err);
                      const args = Array.prototype.slice.call(arguments, 1);
                      // Determine whether to continue the pitching process based on
                      // argument values (as opposed to argument presence) in order
                      // to support synchronous and asynchronous usages.
                      const hasArg = args.some((value) => value !== undefined);
                      if (hasArg) {
                          if (typeof self !== 'undefined' && self.postMessage) {
                              self.postMessage({
                                  type: 'debug',
                                  message: `\u{1F527} Pitch \u{8FD4}\u{56DE}\u{4E86}\u{7ED3}\u{679C}\u{FF0C}\u{8DF3}\u{8F6C}\u{5230} normal loaders`
                              });
                          }
                          loaderContext.loaderIndex--;
                          iterateNormalLoaders(options, loaderContext, args, callback);
                      } else {
                          if (typeof self !== 'undefined' && self.postMessage) {
                              self.postMessage({
                                  type: 'debug',
                                  message: `\u{1F527} Pitch \u{6CA1}\u{6709}\u{8FD4}\u{56DE}\u{7ED3}\u{679C}\u{FF0C}\u{7EE7}\u{7EED} pitching`
                              });
                          }
                          iteratePitchingLoaders(options, loaderContext, callback);
                      }
                  });
              });
          }
          // 创建 LoaderRunner 类
          class LoaderRunner {
              static getContext(resource) {
                  const { path } = parsePathQueryFragment(resource);
                  return dirname(path);
              }
              static runLoaders(options, callback) {
                  // read options
                  const resource = options.resource || "";
                  let loaders = options.loaders || [];
                  const loaderContext = options.context || {};
                  const processResource = options.processResource || ((readResource, context, resource, callback) => {
                      context.addDependency(resource);
                      readResource(resource, callback);
                  }).bind(null, options.readResource || readFile);
                  //
                  const splittedResource = resource && parsePathQueryFragment(resource);
                  const resourcePath = splittedResource ? splittedResource.path : undefined;
                  const resourceQuery = splittedResource ? splittedResource.query : undefined;
                  const resourceFragment = splittedResource ? splittedResource.fragment : undefined;
                  const contextDirectory = resourcePath ? dirname(resourcePath) : null;
                  // execution state
                  let requestCacheable = true;
                  const fileDependencies = [];
                  const contextDependencies = [];
                  const missingDependencies = [];
                  // prepare loader objects
                  loaders = loaders.map(createLoaderObject);
                  loaderContext.context = contextDirectory;
                  loaderContext.loaderIndex = 0;
                  loaderContext.loaders = loaders;
                  loaderContext.resourcePath = resourcePath;
                  loaderContext.resourceQuery = resourceQuery;
                  loaderContext.resourceFragment = resourceFragment;
                  loaderContext.async = null;
                  loaderContext.callback = null;
                  loaderContext.cacheable = function cacheable(flag) {
                      if (flag === false) {
                          requestCacheable = false;
                      }
                  };
                  loaderContext.dependency = loaderContext.addDependency = function addDependency(file) {
                      fileDependencies.push(file);
                  };
                  loaderContext.addContextDependency = function addContextDependency(context) {
                      contextDependencies.push(context);
                  };
                  loaderContext.addMissingDependency = function addMissingDependency(context) {
                      missingDependencies.push(context);
                  };
                  loaderContext.getDependencies = function getDependencies() {
                      return fileDependencies.slice();
                  };
                  loaderContext.getContextDependencies = function getContextDependencies() {
                      return contextDependencies.slice();
                  };
                  loaderContext.getMissingDependencies = function getMissingDependencies() {
                      return missingDependencies.slice();
                  };
                  loaderContext.clearDependencies = function clearDependencies() {
                      fileDependencies.length = 0;
                      contextDependencies.length = 0;
                      missingDependencies.length = 0;
                      requestCacheable = true;
                  };
                  loaderContext.resource = resource;
                  loaderContext.readResource = options.readResource || readFile;
                  Object.defineProperty(loaderContext, "request", {
                      enumerable: true,
                      get() {
                          return loaderContext.loaders.map((loader) => loader.request).concat(loaderContext.resource || "").join("!");
                      }
                  });
                  Object.defineProperty(loaderContext, "remainingRequest", {
                      enumerable: true,
                      get() {
                          if (loaderContext.loaderIndex >= loaderContext.loaders.length - 1 && !loaderContext.resource) {
                              return "";
                          }
                          return loaderContext.loaders.slice(loaderContext.loaderIndex + 1).map((loader) => loader.request).concat(loaderContext.resource || "").join("!");
                      }
                  });
                  Object.defineProperty(loaderContext, "currentRequest", {
                      enumerable: true,
                      get() {
                          return loaderContext.loaders.slice(loaderContext.loaderIndex).map((loader) => loader.request).concat(loaderContext.resource || "").join("!");
                      }
                  });
                  Object.defineProperty(loaderContext, "previousRequest", {
                      enumerable: true,
                      get() {
                          return loaderContext.loaders.slice(0, loaderContext.loaderIndex).map((loader) => loader.request).join("!");
                      }
                  });
                  Object.defineProperty(loaderContext, "query", {
                      enumerable: true,
                      get() {
                          const entry = loaderContext.loaders[loaderContext.loaderIndex];
                          return entry.options && typeof entry.options === "object" ? entry.options : entry.query;
                      }
                  });
                  Object.defineProperty(loaderContext, "data", {
                      enumerable: true,
                      get() {
                          return loaderContext.loaders[loaderContext.loaderIndex].data;
                      }
                  });
                  // finish loader context
                  if (Object.preventExtensions) {
                      Object.preventExtensions(loaderContext);
                  }
                  const processOptions = {
                      resourceBuffer: null,
                      processResource
                  };
                  iteratePitchingLoaders(processOptions, loaderContext, (err, result) => {
                      if (err) {
                          return callback(err, {
                              cacheable: requestCacheable,
                              fileDependencies,
                              contextDependencies,
                              missingDependencies
                          });
                      }
                      callback(null, {
                          result,
                          resourceBuffer: processOptions.resourceBuffer,
                          cacheable: requestCacheable,
                          fileDependencies,
                          contextDependencies,
                          missingDependencies
                      });
                  });
              }
          }
          // 为 Web Worker 环境提供全局访问
          if (typeof self !== 'undefined') {
              self.LoaderRunner = LoaderRunner;
          }


      }),
      395: (function () {

          function handleResult(loader, module, callback) {
              if (typeof module !== "function" && typeof module !== "object") {
                  return callback(new self.LoaderLoadingError(`Module '${loader.path}' is not a loader (export function or es6 module)`));
              }
              loader.normal = typeof module === "function" ? module : module.exports;
              loader.pitch = module.pitch;
              loader.raw = module.raw;
              if (typeof self.postMessage === 'function') {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} handleResult: loader '${loader.path}' \u{8BBE}\u{7F6E}\u{5B8C}\u{6210}`
                  });
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F4CB} loader.normal: ${typeof loader.normal}, loader.pitch: ${typeof loader.pitch}, loader.raw: ${loader.raw}`
                  });
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F4CB} loader \u{5BF9}\u{8C61}: ${JSON.stringify({
                          path: loader.path,
                          query: loader.query,
                          normal: typeof loader.normal,
                          pitch: typeof loader.pitch,
                          raw: loader.raw
                      })}`
                  });
              }
              if (typeof loader.normal !== "function" && typeof loader.pitch !== "function") {
                  return callback(new self.LoaderLoadingError(`Module '${loader.path}' is not a loader (must have normal or pitch function)`));
              }
              callback();
          }
          function loadLoader(loader, callback) {
              if (self.__preloadedModules__ && self.__preloadedModules__[loader.path]) {
                  const loadedModule = self.__preloadedModules__[loader.path];
                  return handleResult(loader, loadedModule, callback);
              }
              self.postMessage({
                  type: 'debug',
                  message: `\u{1F527} \u{521B}\u{5EFA}\u{5185}\u{7F6E}\u{7684} loader \u{5B9E}\u{73B0}: ${loader.path}`
              });
              const builtinModule = createBuiltinLoader(loader.path);
              if (!self.__preloadedModules__) {
                  self.__preloadedModules__ = {};
              }
              self.__preloadedModules__[loader.path] = builtinModule;
              return handleResult(loader, builtinModule, callback);
          }
          function createBuiltinLoader(loaderPath) {
              const loaderName = loaderPath.replace(/^\.\//, '').replace(/\.js$/, '');
              switch (loaderName) {
                  case 'css-loader':
                      return function (source, map, meta) {
                          try {
                              const options = this.getOptions ? this.getOptions() : {};
                              let processedCSS = source;
                              if (!options.keepComments) {
                                  processedCSS = processedCSS.replace(/\/\*[\s\S]*?\*\//g, '');
                              }
                              processedCSS = processedCSS.replace(/^\s*[\r\n]/gm, '') // 移除空行
                                  .replace(/\s+/g, ' ') // 将多个空白字符合并为一个
                                  .trim();
                              if (options.modules) {
                                  const className = 'css_' + Math.random().toString(36).substr(2, 9);
                                  processedCSS = processedCSS.replace(/\.([a-zA-Z][a-zA-Z0-9_-]*)/g, `.${className}_$1`);
                              }
                              // 返回符合 css-loader 格式的结果
                              const result = [
                                  `// Exports`,
                                  `module.exports = ${JSON.stringify(processedCSS)};`
                              ].join('\n');
                              // 使用 callback 返回结果
                              if (this.callback) {
                                  this.callback(null, result);
                                  return;
                              }
                              return result;
                          } catch (error) {
                              // 如果处理失败，返回原始内容
                              const fallbackResult = `module.exports = ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, fallbackResult);
                                  return;
                              }
                              return fallbackResult;
                          }
                      };
                  case 'style-loader':
                      return function (source, map, meta) {
                          try {
                              // style-loader 的简单实现
                              const result = [
                                  `// Style injection`,
                                  `var style = document.createElement('style');`,
                                  `style.textContent = ${JSON.stringify(source)};`,
                                  `document.head.appendChild(style);`,
                                  `module.exports = {};`
                              ].join('\n');
                              if (this.callback) {
                                  this.callback(null, result);
                                  return;
                              }
                              return result;
                          } catch (error) {
                              const fallbackResult = 'module.exports = {};';
                              if (this.callback) {
                                  this.callback(null, fallbackResult);
                                  return;
                              }
                              return fallbackResult;
                          }
                      };
                  case 'babel-loader':
                      return function (source, map, meta) {
                          try {
                              // babel-loader 的简单实现
                              const result = `module.exports = ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, result);
                                  return;
                              }
                              return result;
                          } catch (error) {
                              const fallbackResult = `module.exports = ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, fallbackResult);
                                  return;
                              }
                              return fallbackResult;
                          }
                      };
                  case 'ts-loader':
                      return function (source, map, meta) {
                          try {
                              // ts-loader 的简单实现
                              const result = `module.exports = ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, result);
                                  return;
                              }
                              return result;
                          } catch (error) {
                              const fallbackResult = `module.exports = ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, fallbackResult);
                                  return;
                              }
                              return fallbackResult;
                          }
                      };
                  case 'file-loader':
                      return function (source, map, meta) {
                          try {
                              // file-loader 的简单实现
                              const result = `module.exports = "file://" + ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, result);
                                  return;
                              }
                              return result;
                          } catch (error) {
                              const fallbackResult = `module.exports = "file://" + ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, fallbackResult);
                                  return;
                              }
                              return fallbackResult;
                          }
                      };
                  case 'url-loader':
                      return function (source, map, meta) {
                          try {
                              // url-loader 的简单实现
                              const result = `module.exports = "data:text/plain;base64," + btoa(${JSON.stringify(source)});`;
                              if (this.callback) {
                                  this.callback(null, result);
                                  return;
                              }
                              return result;
                          } catch (error) {
                              const fallbackResult = `module.exports = "data:text/plain;base64," + btoa(${JSON.stringify(source)});`;
                              if (this.callback) {
                                  this.callback(null, fallbackResult);
                                  return;
                              }
                              return fallbackResult;
                          }
                      };
                  default:
                      return function (source, map, meta) {
                          try {
                              // 通用的处理，返回原代码
                              const result = `module.exports = ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, result);
                                  return;
                              }
                              return result;
                          } catch (error) {
                              const fallbackResult = `module.exports = ${JSON.stringify(source)};`;
                              if (this.callback) {
                                  this.callback(null, fallbackResult);
                                  return;
                              }
                              return fallbackResult;
                          }
                      };
              }
          }
          // 为 Web Worker 环境提供全局访问
          if (typeof self !== 'undefined') {
              self.loadLoader = loadLoader;
          }


      }),

  });
  /************************************************************************/
  // The module cache
  var __webpack_module_cache__ = {};

  // The require function
  function __webpack_require__(moduleId) {

      // Check if module is in cache
      var cachedModule = __webpack_module_cache__[moduleId];
      if (cachedModule !== undefined) {
          return cachedModule.exports;
      }
      // Create a new module (and put it into the cache)
      var module = (__webpack_module_cache__[moduleId] = {
          exports: {}
      });
      // Execute the module function
      __webpack_modules__[moduleId](module, module.exports, __webpack_require__);

      // Return the exports of the module
      return module.exports;

  }

  /************************************************************************/
  // webpack/runtime/compat_get_default_export
  (() => {
      // getDefaultExport function for compatibility with non-ESM modules
      __webpack_require__.n = (module) => {
          var getter = module && module.__esModule ?
              () => (module['default']) :
              () => (module);
          __webpack_require__.d(getter, { a: getter });
          return getter;
      };

  })();
  // webpack/runtime/define_property_getters
  (() => {
      __webpack_require__.d = (exports, definition) => {
          for (var key in definition) {
              if (__webpack_require__.o(definition, key) && !__webpack_require__.o(exports, key)) {
                  Object.defineProperty(exports, key, { enumerable: true, get: definition[key] });
              }
          }
      };
  })();
  // webpack/runtime/has_own_property
  (() => {
      __webpack_require__.o = (obj, prop) => (Object.prototype.hasOwnProperty.call(obj, prop))
  })();
  /************************************************************************/
  // This entry needs to be wrapped in an IIFE because it needs to be isolated against other modules in the chunk.
  (() => {
  /* ESM import */var _lib_web_LoaderLoadingError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(91);
  /* ESM import */var _lib_web_LoaderLoadingError_js__WEBPACK_IMPORTED_MODULE_0___default = /*#__PURE__*/__webpack_require__.n(_lib_web_LoaderLoadingError_js__WEBPACK_IMPORTED_MODULE_0__);
  /* ESM import */var _lib_web_loadLoader_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(395);
  /* ESM import */var _lib_web_loadLoader_js__WEBPACK_IMPORTED_MODULE_1___default = /*#__PURE__*/__webpack_require__.n(_lib_web_loadLoader_js__WEBPACK_IMPORTED_MODULE_1__);
  /* ESM import */var _lib_web_LoaderRunner_js__WEBPACK_IMPORTED_MODULE_2__ = __webpack_require__(257);
  /* ESM import */var _lib_web_LoaderRunner_js__WEBPACK_IMPORTED_MODULE_2___default = /*#__PURE__*/__webpack_require__.n(_lib_web_LoaderRunner_js__WEBPACK_IMPORTED_MODULE_2__);
      // importScripts('lib-web/LoaderLoadingError.js');
      // importScripts('lib-web/loadLoader.js');
      // importScripts('lib-web/LoaderRunner.js');



      self.__preloadedModules__ = {};
      self.__fileContents__ = {};
      // 消息处理器
      self.onmessage = function (event) {
          const { messageType, id, payload, type } = event.data;
          if (messageType === 'transform') {
              handleTransform(id, payload);
          } else if (messageType === 'setFileContent') {
              handleSetFileContent(id, payload);
          } else if (type === 'debug') {
              // 处理调试消息
              console.log('Debug:', event.data.message);
          }
      };
      // 处理文件转换
      async function handleTransform(id, payload) {
          try {
              const [source, resourcePath, query, loaders, sourceMap, context] = payload;
              // 将源文件内容存储到 __fileContents__ 中，供 readResource 使用
              self.__fileContents__[resourcePath] = source;
              // 创建完整的 context 对象，包含所有必要的方法
              const loaderContext = {
                  cwd: typeof context === 'string' ? context : (context === null || context === void 0 ? void 0 : context.cwd) || '/',
                  addDependency: function (file) {
                      // 添加文件依赖
                      if (!this.dependencies) this.dependencies = [];
                      this.dependencies.push(file);
                  },
                  addContextDependency: function (context) {
                      // 添加上下文依赖
                      if (!this.contextDependencies) this.contextDependencies = [];
                      this.contextDependencies.push(context);
                  },
                  addMissingDependency: function (context) {
                      // 添加缺失的依赖
                      if (!this.missingDependencies) this.missingDependencies = [];
                      this.missingDependencies.push(context);
                  },
                  getDependencies: function () {
                      return this.dependencies || [];
                  },
                  getContextDependencies: function () {
                      return this.contextDependencies || [];
                  },
                  getMissingDependencies: function () {
                      return this.missingDependencies || [];
                  },
                  clearDependencies: function () {
                      this.dependencies = [];
                      this.contextDependencies = [];
                      this.missingDependencies = [];
                  },
                  cacheable: function (flag) {
                      this.cacheableFlag = flag !== false;
                  }
              };
              // 使用 LoaderRunner 执行 loader 转换
              self.postMessage({
                  type: 'debug',
                  message: "\uD83D\uDD27 \u5F00\u59CB\u8C03\u7528 LoaderRunner.runLoaders..."
              });
              self.postMessage({
                  type: 'debug',
                  message: `\u{1F4CB} \u{4F20}\u{5165}\u{7684} loaders: ${JSON.stringify(loaders.map((loader) => ({
                      loader: loader.loader,
                      options: loader.options
                  })))}`
              });
              self.LoaderRunner.runLoaders({
                  resource: resourcePath,
                  loaders: loaders.map((loader) => ({
                      loader: loader.loader,
                      options: loader.options || {},
                      path: loader.loader,
                      query: loader.options || {}
                  })),
                  context: loaderContext,
                  processResource: (readResource, context, resource, callback) => {
                      // 自定义 processResource 函数，确保 context 有 addDependency 方法
                      if (typeof context.addDependency === 'function') {
                          context.addDependency(resource);
                      }
                      // 直接返回源文件内容，而不是调用 readResource
                      callback(null, source);
                  },
                  readResource: (path, callback) => {
                      // 从预加载的文件内容中读取
                      if (self.__fileContents__[path]) {
                          callback(null, self.__fileContents__[path]);
                      } else {
                          callback(new Error(`File not found: ${path}`));
                      }
                  }
              }, (err, result) => {
                  self.postMessage({
                      type: 'debug',
                      message: `\u{1F527} LoaderRunner \u{56DE}\u{8C03}\u{88AB}\u{8C03}\u{7528}: err=${err ? 'yes' : 'no'}, result=${result ? 'yes' : 'no'}`
                  });
                  if (result) {
                      self.postMessage({
                          type: 'debug',
                          message: `\u{1F4CB} result.result: ${JSON.stringify(result.result)}`
                      });
                      self.postMessage({
                          type: 'debug',
                          message: `\u{1F4CB} result.resourceBuffer: ${result.resourceBuffer ? 'yes' : 'no'}`
                      });
                  }
                  if (err) {
                      self.postMessage({
                          id,
                          error: {
                              message: err.message,
                              stack: err.stack
                          }
                      });
                  } else {
                      self.postMessage({
                          id,
                          result: {
                              source: result.result[0],
                              map: sourceMap ? result.resourceBuffer : null
                          }
                      });
                  }
              });
          } catch (error) {
              self.postMessage({
                  id,
                  error: {
                      message: error.message,
                      stack: error.stack
                  }
              });
          }
      }
      // 处理文件内容设置
      function handleSetFileContent(id, payload) {
          try {
              const { path, content } = payload;
              self.__fileContents__[path] = content;
              self.postMessage({
                  id,
                  result: {
                      success: true,
                      path
                  }
              });
          } catch (error) {
              self.postMessage({
                  id,
                  error: {
                      message: error.message,
                      stack: error.stack
                  }
              });
          }
      }
      self.postMessage({
          type: 'ready',
          message: 'Worker initialized with built-in loader support'
      });

  })();

})()
  ;