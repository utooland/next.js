import { workerData } from 'worker_threads'

// This is needed for hijack process.cwd in worker, it's safe because
// process in worker thread is isolated to schedule thread.
if (!workerData.hasOwnProperty('cwd')) {
  throw new Error('cwd not set in loader worker thread')
}
process.cwd = () => workerData.cwd

// @ts-ignore
process.turbopack = {}
