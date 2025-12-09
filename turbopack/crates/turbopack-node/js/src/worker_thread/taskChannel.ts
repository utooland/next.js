import { structuredError } from '../error'

export interface Binding {
  recvMessageInWorker(workerId: number): Promise<{
    taskId: number
    message: string
  }>
  sendTaskMessage(taskId: number, message: string): Promise<void>
  workerCreated(workerId: number): void
}

// Export this, maybe in the future, we can add an implementation via web worker on browser
export class TaskChannel {
  static nextId = 1
  static requests = new Map()

  constructor(
    private binding: Binding,
    private taskId: number
  ) {}

  async sendInfo(message: any) {
    return await this.binding.sendTaskMessage(
      this.taskId,
      JSON.stringify({
        type: 'info',
        data: message,
      })
    )
  }

  async sendRequest(message: any) {
    const id = TaskChannel.nextId++
    let resolve, reject
    const promise = new Promise((res, rej) => {
      resolve = res
      reject = rej
    })
    TaskChannel.requests.set(id, { resolve, reject })
    return await this.binding
      .sendTaskMessage(
        this.taskId,
        JSON.stringify({ type: 'request', id, data: message })
      )
      .then(() => promise)
  }

  async sendError(error: Error) {
    try {
      await this.binding.sendTaskMessage(
        this.taskId,
        JSON.stringify({
          type: 'error',
          ...structuredError(error),
        })
      )
    } catch (err) {
      // There's nothing we can do about errors that happen after this point, we can't tell anyone
      // about them.
      console.error('failed to send error back to rust:', err)
    }
  }
}
