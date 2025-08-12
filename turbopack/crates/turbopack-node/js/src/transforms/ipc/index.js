"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.IPC = void 0;
exports.structuredError = structuredError;
const node_net_1 = require("node:net");
const node_stream_1 = require("node:stream");
const stacktrace_parser_1 = require("../compiled/stacktrace-parser");
const error_1 = require("./error");
function structuredError(e) {
    e = (0, error_1.getProperError)(e);
    return {
        name: e.name,
        message: e.message,
        stack: typeof e.stack === 'string' ? (0, stacktrace_parser_1.parse)(e.stack) : [],
        cause: e.cause ? structuredError((0, error_1.getProperError)(e.cause)) : undefined,
    };
}
function createIpc(port) {
    const socket = (0, node_net_1.createConnection)({
        port,
        host: '127.0.0.1',
    });
    /**
     * A writable stream that writes to the socket.
     * We don't write directly to the socket because we need to
     * handle backpressure and wait for the socket to be drained
     * before writing more data.
     */
    const socketWritable = new node_stream_1.Writable({
        write(chunk, _enc, cb) {
            if (socket.write(chunk)) {
                cb();
            }
            else {
                socket.once('drain', cb);
            }
        },
        final(cb) {
            socket.end(cb);
        },
    });
    const packetQueue = [];
    const recvPromiseResolveQueue = [];
    function pushPacket(packet) {
        const recvPromiseResolve = recvPromiseResolveQueue.shift();
        if (recvPromiseResolve != null) {
            recvPromiseResolve(JSON.parse(packet.toString('utf8')));
        }
        else {
            packetQueue.push(packet);
        }
    }
    let state = { type: 'waiting' };
    let buffer = Buffer.alloc(0);
    socket.once('connect', () => {
        socket.on('data', (chunk) => {
            buffer = Buffer.concat([buffer, chunk]);
            loop: while (true) {
                switch (state.type) {
                    case 'waiting': {
                        if (buffer.length >= 4) {
                            const length = buffer.readUInt32BE(0);
                            buffer = buffer.subarray(4);
                            state = { type: 'packet', length };
                        }
                        else {
                            break loop;
                        }
                        break;
                    }
                    case 'packet': {
                        if (buffer.length >= state.length) {
                            const packet = buffer.subarray(0, state.length);
                            buffer = buffer.subarray(state.length);
                            state = { type: 'waiting' };
                            pushPacket(packet);
                        }
                        else {
                            break loop;
                        }
                        break;
                    }
                    default:
                        invariant(state, (state) => `Unknown state type: ${state?.type}`);
                }
            }
        });
    });
    // When the socket is closed, this process is no longer needed.
    // This might happen e. g. when parent process is killed or
    // node.js pool is garbage collected.
    socket.once('close', () => {
        process.exit(0);
    });
    // TODO(lukesandberg): some of the messages being sent are very large and contain lots
    //  of redundant information.  Consider adding gzip compression to our stream.
    function doSend(message) {
        return new Promise((resolve, reject) => {
            // Reserve 4 bytes for our length prefix, we will over-write after encoding.
            const packet = Buffer.from('0000' + message, 'utf8');
            packet.writeUInt32BE(packet.length - 4, 0);
            socketWritable.write(packet, (err) => {
                process.stderr.write(`TURBOPACK_OUTPUT_D\n`);
                process.stdout.write(`TURBOPACK_OUTPUT_D\n`);
                if (err != null) {
                    reject(err);
                }
                else {
                    resolve();
                }
            });
        });
    }
    function send(message) {
        return doSend(JSON.stringify(message));
    }
    function sendReady() {
        return doSend('');
    }
    return {
        async recv() {
            const packet = packetQueue.shift();
            if (packet != null) {
                return JSON.parse(packet.toString('utf8'));
            }
            const result = await new Promise((resolve) => {
                recvPromiseResolveQueue.push((result) => {
                    resolve(result);
                });
            });
            return result;
        },
        send(message) {
            return send(message);
        },
        sendReady,
        async sendError(error) {
            try {
                await send({
                    type: 'error',
                    ...structuredError(error),
                });
            }
            catch (err) {
                console.error('failed to send error back to rust:', err);
                // ignore and exit anyway
                process.exit(1);
            }
            process.exit(0);
        },
    };
}
const PORT = process.argv[2];
exports.IPC = createIpc(parseInt(PORT, 10));
process.on('uncaughtException', (err) => {
    exports.IPC.sendError(err);
});
const improveConsole = (name, stream, addStack) => {
    // @ts-ignore
    const original = console[name];
    // @ts-ignore
    const stdio = process[stream];
    // @ts-ignore
    console[name] = (...args) => {
        stdio.write(`TURBOPACK_OUTPUT_B\n`);
        original(...args);
        if (addStack) {
            const stack = new Error().stack?.replace(/^.+\n.+\n/, '') + '\n';
            stdio.write('TURBOPACK_OUTPUT_S\n');
            stdio.write(stack);
        }
        stdio.write('TURBOPACK_OUTPUT_E\n');
    };
};
improveConsole('error', 'stderr', true);
improveConsole('warn', 'stderr', true);
improveConsole('count', 'stdout', true);
improveConsole('trace', 'stderr', false);
improveConsole('log', 'stdout', true);
improveConsole('group', 'stdout', true);
improveConsole('groupCollapsed', 'stdout', true);
improveConsole('table', 'stdout', true);
improveConsole('debug', 'stdout', true);
improveConsole('info', 'stdout', true);
improveConsole('dir', 'stdout', true);
improveConsole('dirxml', 'stdout', true);
improveConsole('timeEnd', 'stdout', true);
improveConsole('timeLog', 'stdout', true);
improveConsole('timeStamp', 'stdout', true);
improveConsole('assert', 'stderr', true);
/**
 * Utility function to ensure all variants of an enum are handled.
 */
function invariant(never, computeMessage) {
    throw new Error(`Invariant: ${computeMessage(never)}`);
}
