import { workerData } from 'worker_threads'

// This is needed for hijack process.cwd in worker, it's safe because
// process in worker thread is isolated to schedule thread.
process.cwd = () => workerData.cwd

// @ts-ignore
process.turbopack = {}
