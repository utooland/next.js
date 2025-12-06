import { threadId as workerId, workerData, parentPort } from 'worker_threads'
import { structuredError } from '../error'
import type { Channel } from '../types'
import { Binding, TaskChannel } from './taskChannel'

const binding: Binding = require(
  /* turbopackIgnore: true */ workerData.bindingPath
)

const KillMsg = '__kill__'

let willKill = false

parentPort!.on('message', (msg) => {
  if (msg === KillMsg) {
    willKill = true
  }
})

export const run = async (
  moduleFactory: () => Promise<{
    init?: () => Promise<void>
    default: (channel: Channel<any, any>, ...deserializedArgs: any[]) => any
  }>
) => {
  let getValue: (channel: Channel<any, any>, ...deserializedArgs: any[]) => any

  let isRunning = false
  let runningTask: Promise<void> | undefined

  const run = async (taskId: number, args: string[]) => {
    try {
      if (typeof getValue !== 'function') {
        const module = await moduleFactory()
        if (typeof module.init === 'function') {
          await module.init()
        }
        getValue = module.default
      }
      const value = await getValue(new TaskChannel(binding, taskId), ...args)
      await binding.sendTaskMessage(
        taskId,
        JSON.stringify({
          type: 'end',
          data: value === undefined ? undefined : JSON.stringify(value),
          duration: 0,
        })
      )
    } catch (err) {
      await binding.sendTaskMessage(
        taskId,
        JSON.stringify({
          type: 'error',
          ...structuredError(err as Error),
        })
      )
    }
    isRunning = false
    runningTask = undefined
  }

  const loop = async () => {
    let taskId: number | undefined
    let msg_str: string

    if (isRunning) {
      msg_str = await binding.recvMessageInWorker(workerId)
    } else {
      taskId = await binding.recvWorkerRequest(workerData.poolId)
      await binding.notifyWorkerAck(taskId, workerId)
      msg_str = await binding.recvMessageInWorker(workerId)
    }

    const msg = JSON.parse(msg_str) as
      | {
          type: 'evaluate'
          args: string[]
        }
      | {
          type: 'result'
          id: number
          error?: string
          data?: any
        }

    switch (msg.type) {
      case 'evaluate': {
        if (!isRunning && taskId !== undefined) {
          isRunning = true
          runningTask = run(taskId, msg.args)
        }
        break
      }
      case 'result': {
        const request = TaskChannel.requests.get(msg.id)
        if (request) {
          TaskChannel.requests.delete(msg.id)
          if (msg.error) {
            // Need to reject at next macro task queue, because some rejection callbacks is not registered when executing to here,
            // that will cause the error be propergated to schedule thread, then causing panic.
            // The situation always happen when using sass-loader, it will try to resolve many posible dependencies,
            // some of them may fail with error.
            setTimeout(() => request.reject(new Error(msg.error)), 0)
          } else {
            request.resolve(msg.data)
          }
        }
        break
      }
      default: {
        console.error('unexpected message type', (msg as any).type)
      }
    }
  }

  while (true) {
    if (willKill) {
      if (runningTask) {
        await runningTask
      }
      parentPort!.postMessage(KillMsg)
      return
    }

    const loopTask = loop()

    if (!isRunning) {
      runningTask = loopTask
    }

    await loopTask
  }
}
