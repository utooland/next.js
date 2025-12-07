import { Worker } from 'worker_threads'
import type {
  WorkerCreationParams,
  WorkerTermination,
} from './generated-native'

const loaderWorkers: Record<string, Array<Worker>> = {}

export async function runLoaderWorkerPool(
  binding: typeof import('./generated-native'),
  bindingPath: string
) {
  binding.registerWorkerCreator((request: WorkerCreationParams) => {
    const { options, taskId } = request
    const { filename, cwd } = options

    const worker = new Worker(filename, {
      workerData: {
        poolId: filename,
        bindingPath,
        cwd,
      },
    })

    const workers = loaderWorkers[filename] || (loaderWorkers[filename] = [])
    workers.push(worker)

    binding.workerCreated(taskId, worker.threadId)
  })

  binding.registerWorkerTerminator((request: WorkerTermination) => {
    const { filename, workerId } = request
    const workers = loaderWorkers[filename]
    const workerIdx = workers.findIndex(
      (worker) => worker.threadId === workerId
    )
    if (workerIdx > -1) {
      const workersToKill = workers.splice(workerIdx, 1)
      workersToKill.forEach((worker) => worker.terminate())
    }
  })
}
