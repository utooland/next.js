// @eslint-ignore
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.default = void 0;
const path_1 = require("path");
const stacktrace_parser_1 = require("../compiled/stacktrace-parser");
const ipc_1 = require("../ipc");
// Import loader-runner for WebWorker environment
const { runLoaders, } = require('@vercel/turbopack/loader-runner');
// WebWorker doesn't have process.cwd(), so we'll use a default or receive it from the message
let contextDir = '/';
const LogType = Object.freeze({
    error: 'error',
    warn: 'warn',
    info: 'info',
    log: 'log',
    debug: 'debug',
    trace: 'trace',
    group: 'group',
    groupCollapsed: 'groupCollapsed',
    groupEnd: 'groupEnd',
    profile: 'profile',
    profileEnd: 'profileEnd',
    time: 'time',
    clear: 'clear',
    status: 'status',
});
const loaderFlag = 'LOADER_EXECUTION';
const cutOffByFlag = (stack, flag) => {
    const errorStack = stack.split('\n');
    for (let i = 0; i < errorStack.length; i++) {
        if (errorStack[i].includes(flag)) {
            errorStack.length = i;
        }
    }
    return errorStack.join('\n');
};
const cutOffLoaderExecution = (stack) => cutOffByFlag(stack, loaderFlag);
class DummySpan {
    traceChild() {
        return new DummySpan();
    }
    traceFn(fn) {
        return fn(this);
    }
    async traceAsyncFn(fn) {
        return await fn(this);
    }
    stop() {
        return;
    }
}
// WebWorker-specific IPC implementation
class WebWorkerIpc {
    constructor() {
        this.envVariables = new Set();
    }
    sendInfo(info) {
        // In WebWorker, we can log info or store it for later transmission
        if (info.type === 'log') {
            info.logs.forEach((log) => {
                console[log.logType]?.(...log.args);
            });
        }
    }
    sendError(error) {
        console.error('WebWorker Error:', error);
    }
    async sendRequest(request) {
        // In WebWorker environment, we need to handle resolution differently
        // For now, we'll use a simplified approach
        if (request.type === 'resolve') {
            try {
                // Try to resolve using require.resolve in the WebWorker context
                const resolved = __turbopack_external_require__.resolve(request.request, {
                    paths: [request.lookupPath],
                });
                return { path: resolved };
            }
            catch (e) {
                // Fallback to the original request if resolution fails
                return { path: request.request };
            }
        }
        throw new Error(`Unsupported request type: ${request.type}`);
    }
    addEnvVariable(name) {
        this.envVariables.add(name);
    }
    getReadEnvVariables() {
        return Array.from(this.envVariables);
    }
}
const transform = (content, name, query, loaders, sourceMap, cwd) => {
    return new Promise((resolve, reject) => {
        // Update context directory if provided
        if (cwd) {
            contextDir = cwd;
        }
        const resource = (0, path_1.resolve)(contextDir, name);
        const resourceDir = (0, path_1.dirname)(resource);
        const ipc = new WebWorkerIpc();
        const loadersWithOptions = loaders.map((loader) => typeof loader === 'string' ? { loader, options: {} } : loader);
        const logs = [];
        runLoaders({
            resource: resource + query,
            context: {
                _module: {
                    // For debugging purpose, if someone find context is not full compatible to
                    // webpack they can guess this comes from turbopack WebWorker
                    __reserved: 'TurbopackWebWorkerContext',
                },
                currentTraceSpan: new DummySpan(),
                rootContext: contextDir,
                sourceMap,
                getOptions() {
                    const entry = this.loaders[this.loaderIndex];
                    return entry.options && typeof entry.options === 'object'
                        ? entry.options
                        : {};
                },
                getResolve: (options) => {
                    const rustOptions = {
                        aliasFields: undefined,
                        conditionNames: undefined,
                        noPackageJson: false,
                        extensions: undefined,
                        mainFields: undefined,
                        noExportsField: false,
                        mainFiles: undefined,
                        noModules: false,
                        preferRelative: false,
                    };
                    // Apply resolve options similar to the original implementation
                    if (options.alias) {
                        if (!Array.isArray(options.alias) || options.alias.length > 0) {
                            throw new Error('alias resolve option is not supported');
                        }
                    }
                    if (options.aliasFields) {
                        if (!Array.isArray(options.aliasFields)) {
                            throw new Error('aliasFields resolve option must be an array');
                        }
                        rustOptions.aliasFields = options.aliasFields;
                    }
                    if (options.conditionNames) {
                        if (!Array.isArray(options.conditionNames)) {
                            throw new Error('conditionNames resolve option must be an array');
                        }
                        rustOptions.conditionNames = options.conditionNames;
                    }
                    if (options.descriptionFiles) {
                        if (!Array.isArray(options.descriptionFiles) ||
                            options.descriptionFiles.length > 0) {
                            throw new Error('descriptionFiles resolve option is not supported');
                        }
                        rustOptions.noPackageJson = true;
                    }
                    if (options.extensions) {
                        if (!Array.isArray(options.extensions)) {
                            throw new Error('extensions resolve option must be an array');
                        }
                        rustOptions.extensions = options.extensions;
                    }
                    if (options.mainFields) {
                        if (!Array.isArray(options.mainFields)) {
                            throw new Error('mainFields resolve option must be an array');
                        }
                        rustOptions.mainFields = options.mainFields;
                    }
                    if (options.exportsFields) {
                        if (!Array.isArray(options.exportsFields) ||
                            options.exportsFields.length > 0) {
                            throw new Error('exportsFields resolve option is not supported');
                        }
                        rustOptions.noExportsField = true;
                    }
                    if (options.mainFiles) {
                        if (!Array.isArray(options.mainFiles)) {
                            throw new Error('mainFiles resolve option must be an array');
                        }
                        rustOptions.mainFiles = options.mainFiles;
                    }
                    if (options.modules) {
                        if (!Array.isArray(options.modules) ||
                            options.modules.length > 0) {
                            throw new Error('modules resolve option is not supported');
                        }
                        rustOptions.noModules = true;
                    }
                    if (options.restrictions) {
                        // TODO This is ignored for now
                    }
                    if (options.dependencyType) {
                        // TODO This is ignored for now
                    }
                    if (options.preferRelative) {
                        if (typeof options.preferRelative !== 'boolean') {
                            throw new Error('preferRelative resolve option must be a boolean');
                        }
                        rustOptions.preferRelative = options.preferRelative;
                    }
                    return (lookupPath, request, callback) => {
                        const promise = ipc
                            .sendRequest({
                            type: 'resolve',
                            options: rustOptions,
                            lookupPath: lookupPath,
                            request,
                        })
                            .then((result) => {
                            if (result && typeof result.path === 'string') {
                                return result.path;
                            }
                            else {
                                throw Error('Expected { path: string } from resolve request');
                            }
                        });
                        if (callback) {
                            promise
                                .then((result) => callback(undefined, result), (err) => callback(err))
                                .catch((err) => {
                                ipc.sendError(err);
                            });
                        }
                        else {
                            return promise;
                        }
                    };
                },
                emitWarning: makeErrorEmitter('warning', ipc),
                emitError: makeErrorEmitter('error', ipc),
                getLogger(name) {
                    const logFn = (logType, ...args) => {
                        let trace;
                        switch (logType) {
                            case LogType.warn:
                            case LogType.error:
                            case LogType.trace:
                            case LogType.debug:
                                trace = (0, stacktrace_parser_1.parse)(cutOffLoaderExecution(new Error('Trace').stack)
                                    .split('\n')
                                    .slice(3)
                                    .join('\n'));
                                break;
                            default:
                                break;
                        }
                        // Batch logs messages to be sent at the end
                        logs.push({
                            time: Date.now(),
                            logType,
                            args,
                            trace,
                        });
                    };
                    let timers;
                    let timersAggregates;
                    // See https://github.com/webpack/webpack/blob/a48c34b34d2d6c44f9b2b221d7baf278d34ac0be/lib/logging/Logger.js#L8
                    return {
                        error: logFn.bind(this, LogType.error),
                        warn: logFn.bind(this, LogType.warn),
                        info: logFn.bind(this, LogType.info),
                        log: logFn.bind(this, LogType.log),
                        debug: logFn.bind(this, LogType.debug),
                        assert: (assertion, ...args) => {
                            if (!assertion) {
                                logFn(LogType.error, ...args);
                            }
                        },
                        trace: logFn.bind(this, LogType.trace),
                        clear: logFn.bind(this, LogType.clear),
                        status: logFn.bind(this, LogType.status),
                        group: logFn.bind(this, LogType.group),
                        groupCollapsed: logFn.bind(this, LogType.groupCollapsed),
                        groupEnd: logFn.bind(this, LogType.groupEnd),
                        profile: logFn.bind(this, LogType.profile),
                        profileEnd: logFn.bind(this, LogType.profileEnd),
                        time: (label) => {
                            timers = timers || new Map();
                            // WebWorker doesn't have process.hrtime, use performance.now()
                            const now = performance.now();
                            timers.set(label, [Math.floor(now / 1000), (now % 1000) * 1e6]);
                        },
                        timeLog: (label) => {
                            const prev = timers && timers.get(label);
                            if (!prev) {
                                throw new Error(`No such label '${label}' for WebpackLogger.timeLog()`);
                            }
                            const now = performance.now();
                            const current = [
                                Math.floor(now / 1000),
                                (now % 1000) * 1e6,
                            ];
                            const time = [
                                current[0] - prev[0],
                                current[1] - prev[1],
                            ];
                            if (time[1] < 0) {
                                time[0] -= 1;
                                time[1] += 1e9;
                            }
                            logFn(LogType.time, [label, ...time]);
                        },
                        timeEnd: (label) => {
                            const prev = timers && timers.get(label);
                            if (!prev) {
                                throw new Error(`No such label '${label}' for WebpackLogger.timeEnd()`);
                            }
                            const now = performance.now();
                            const current = [
                                Math.floor(now / 1000),
                                (now % 1000) * 1e6,
                            ];
                            const time = [
                                current[0] - prev[0],
                                current[1] - prev[1],
                            ];
                            if (time[1] < 0) {
                                time[0] -= 1;
                                time[1] += 1e9;
                            }
                            timers.delete(label);
                            logFn(LogType.time, [label, ...time]);
                        },
                        timeAggregate: (label) => {
                            const prev = timers && timers.get(label);
                            if (!prev) {
                                throw new Error(`No such label '${label}' for WebpackLogger.timeAggregate()`);
                            }
                            const now = performance.now();
                            const current = [
                                Math.floor(now / 1000),
                                (now % 1000) * 1e6,
                            ];
                            const time = [
                                current[0] - prev[0],
                                current[1] - prev[1],
                            ];
                            if (time[1] < 0) {
                                time[0] -= 1;
                                time[1] += 1e9;
                            }
                            timers.delete(label);
                            timersAggregates = timersAggregates || new Map();
                            const currentAgg = timersAggregates.get(label);
                            if (currentAgg !== undefined) {
                                if (time[1] + currentAgg[1] > 1e9) {
                                    time[0] += currentAgg[0] + 1;
                                    time[1] = time[1] - 1e9 + currentAgg[1];
                                }
                                else {
                                    time[0] += currentAgg[0];
                                    time[1] += currentAgg[1];
                                }
                            }
                            timersAggregates.set(label, time);
                        },
                        timeAggregateEnd: (label) => {
                            if (timersAggregates === undefined)
                                return;
                            const time = timersAggregates.get(label);
                            if (time === undefined)
                                return;
                            timersAggregates.delete(label);
                            logFn(LogType.time, [label, ...time]);
                        },
                    };
                },
            },
            loaders: loadersWithOptions.map((loader) => ({
                loader: __turbopack_external_require__.resolve(loader.loader, {
                    paths: [resourceDir],
                }),
                options: loader.options,
            })),
            readResource: (_filename, callback) => {
                // TODO assuming that filename === resource, but loaders might change that
                let data = typeof content === 'string'
                    ? Buffer.from(content, 'utf-8')
                    : Buffer.from(content.binary, 'base64');
                callback(null, data);
            },
        }, (err, result) => {
            if (logs.length) {
                ipc.sendInfo({ type: 'log', logs: logs });
                logs.length = 0;
            }
            ipc.sendInfo({
                type: 'dependencies',
                envVariables: ipc.getReadEnvVariables(),
                filePaths: result?.fileDependencies || [],
                directories: (result?.contextDependencies || []).map((dep) => [
                    dep,
                    '**',
                ]),
            });
            if (err)
                return reject(err);
            if (!result?.result)
                return reject(new Error('No result from loaders'));
            const [source, map] = result.result;
            resolve({
                source: Buffer.isBuffer(source)
                    ? { binary: source.toString('base64') }
                    : source,
                map: typeof map === 'string'
                    ? map
                    : typeof map === 'object'
                        ? JSON.stringify(map)
                        : undefined,
            });
        });
    });
};
exports.default = transform;
function makeErrorEmitter(severity, ipc) {
    return function (error) {
        ipc.sendInfo({
            type: 'emittedError',
            severity: severity,
            error: (0, ipc_1.structuredError)(error),
        });
    };
}
// WebWorker message handler
if (typeof self !== 'undefined' && typeof importScripts === 'function') {
    self.onmessage = async function (event) {
        try {
            const data = JSON.parse(event.data);
            // Transform the result to match Rust's expected format
            const result = await transform(data.content, data.name, data.query || '', data.loaders || [], data.sourceMap || false, data.cwd);
            // Convert the result format to match WebpackLoadersProcessingResult in Rust
            const processedResult = {
                source: typeof result.source === 'object' && result.source.binary
                    ? result.source.binary
                    : result.source || '',
                map: result.map
                    ? typeof result.map === 'string'
                        ? JSON.parse(result.map)
                        : result.map
                    : null,
                assets: result.assets || null,
                warnings: result.warnings || null,
                errors: result.errors || null,
            };
            self.postMessage(JSON.stringify(processedResult));
        }
        catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            // Return error in the expected format
            const errorResult = {
                source: '',
                map: null,
                assets: null,
                warnings: null,
                errors: [errorMessage],
            };
            self.postMessage(JSON.stringify(errorResult));
        }
    };
}
