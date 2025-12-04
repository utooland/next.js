import { Worker } from 'worker_threads'

const loaderWorkers: Record<string, Array<Worker>> = {}

const KillMsg = '__kill__'

async function gracefullyKillWorker(worker: Worker) {
  await new Promise<void>((resolve) => {
    let timeout: NodeJS.Timeout
    const onMessage = (msg: any) => {
      if (msg === KillMsg) {
        clearTimeout(timeout)
        worker.off('message', onMessage)
        resolve()
      }
    }
    worker.on('message', onMessage)
    worker.postMessage(KillMsg)
    timeout = setTimeout(() => {
      worker.off('message', onMessage)
      resolve()
    }, 1000)
  })
  await worker.terminate()
}

export async function createOrScalePool(
  binding: typeof import('./generated-native'),
  bindingPath: string
) {
  while (true) {
    try {
      let poolOptions = await binding.recvPoolRequest()
      const { filename, maxConcurrency, env } = poolOptions
      const workers = loaderWorkers[filename] || (loaderWorkers[filename] = [])
      if (workers.length < maxConcurrency) {
        for (let i = workers.length; i < maxConcurrency; i++) {
          const worker = new Worker(filename, {
            workerData: {
              poolId: filename,
              bindingPath,
            },
            env,
          })
          workers.push(worker)
        }
      } else if (workers.length > maxConcurrency) {
        const workersToKill = workers.splice(0, workers.length - maxConcurrency)
        workersToKill.forEach(gracefullyKillWorker)
      }
    } catch (_) {
      // rust channel closed, do nothing
      return
    }
  }
}

export async function waitingForWorkerTermination(
  binding: typeof import('./generated-native')
) {
  while (true) {
    try {
      const { filename, workerId } = await binding.recvWorkerTermination()
      const workers = loaderWorkers[filename]
      const workerIdx = workers.findIndex(
        (worker) => worker.threadId === workerId
      )
      if (workerIdx > -1) {
        const workersToKill = workers.splice(workerIdx, 1)
        workersToKill.forEach(gracefullyKillWorker)
      }
    } catch (_) {
      // rust channel closed, do nothing
      return
    }
  }
}
