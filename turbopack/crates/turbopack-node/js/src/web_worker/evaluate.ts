import { Binding, TaskChannel } from '../worker_threads/taskChannel'
import { structuredError } from '../error'
import type { Channel } from '../types'

export type Self = DedicatedWorkerGlobalScope & {
  workerData: {
    workerId: number
    poolId: string
    cwd: string
    env?: Record<string, string>
    binding: Binding
    readFile(path: string, encoding?: 'utf8'): Promise<string>
  }
}

export declare const self: Self
// @ts-ignore
const { workerId, poolId } = self.workerData

let binding: Binding = self.workerData.binding

export const run = async (
  moduleFactory: () => Promise<{
    init?: () => Promise<void>
    default: (channel: Channel<any, any>, ...deserializedArgs: any[]) => any
  }>
) => {
  let getValue: (channel: Channel<any, any>, ...deserializedArgs: any[]) => any

  let isRunning = false

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
  }

  while (true) {
    const taskId = await binding.recvWorkerRequest(poolId)

    await binding.notifyWorkerAck(taskId, workerId)

    const msg_str = await binding.recvMessageInWorker(workerId)

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
        if (!isRunning) {
          isRunning = true
          run(taskId, msg.args)
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
            // some of then will got a failure.
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
}
