import { Worker } from 'worker_threads'
import type { WorkerCreation, WorkerTermination } from './generated-native'

const loaderWorkers: Record<string, Map<number, Worker>> = {}

export async function runLoaderWorkerPool(
  binding: typeof import('./generated-native'),
  bindingPath: string
) {
  binding.registerWorkerScheduler(
    (creation: WorkerCreation) => {
      const { filename, cwd } = creation

      let poolId = `${cwd}:${filename}`

      const worker = new Worker(filename, {
        workerData: {
          poolId,
          bindingPath,
          cwd,
        },
      })

      const workers =
        loaderWorkers[poolId] || (loaderWorkers[poolId] = new Map())

      workers.set(worker.threadId, worker)
    },
    (termination: WorkerTermination) => {
      const { filename, workerId } = termination
      const workers = loaderWorkers[filename]
      workers.get(workerId)?.terminate()
    }
  )
}
