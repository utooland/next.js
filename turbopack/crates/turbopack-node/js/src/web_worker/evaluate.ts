import {
  Binding,
  TaskChannel,
  TEXT_DECODER,
  TEXT_ENCODER,
} from '../worker_thread/taskChannel'
import { structuredError } from '../error'
import type { Channel } from '../types'
import { workerData, threadId as workerId } from 'worker_threads'

export type Self = DedicatedWorkerGlobalScope

export declare const self: Self

let binding: Binding = workerData.binding

binding.workerCreated(workerId)

export const run = async (
  moduleFactory: () => Promise<{
    init?: () => Promise<void>
    default: (channel: Channel<any, any>, ...deserializedArgs: any[]) => any
  }>
) => {
  let getValue: (channel: Channel<any, any>, ...deserializedArgs: any[]) => any

  let isRunning = false
  const queue: Array<{ taskId: number; args: string[] }> = []

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
      await binding.sendTaskMessage({
        taskId,
        data: TEXT_ENCODER.encode(
          JSON.stringify({
            type: 'end',
            data: value === undefined ? undefined : JSON.stringify(value),
            duration: 0,
          })
        ),
      })
    } catch (err) {
      await binding.sendTaskMessage({
        taskId,
        data: TEXT_ENCODER.encode(
          JSON.stringify({
            type: 'error',
            ...structuredError(err as Error),
          })
        ),
      })
    }
    if (queue.length > 0) {
      const next = queue.shift()!
      run(next.taskId, next.args)
    } else {
      isRunning = false
    }
  }

  while (true) {
    const { taskId, data: msg_buf } =
      await binding.recvTaskMessageInWorker(workerId)

    const msg = JSON.parse(TEXT_DECODER.decode(msg_buf)) as
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
        } else {
          queue.push({ taskId, args: msg.args })
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
            // The situation always happen when using sass-loader, it will try to resolve many possible dependencies,
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
