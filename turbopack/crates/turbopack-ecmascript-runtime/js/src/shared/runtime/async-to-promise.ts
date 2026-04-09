/// <reference path="./runtime-utils.ts" />

/**
 * Executes a Generator function that was compiled from an `async` function.
 *
 * Why does this return a standard function and use Promises recursively, instead of being a
 * generator itself?
 *
 * Turbopack's core module execution pipeline (`__turbopack_context__.a`) is entirely synchronous.
 * It invokes the wrapper function expecting immediate execution. If this returned a generator,
 * the module executor would simply receive an Iterator object and exit without ever calling
 * `.next()` on it.
 *
 * Instead, this helper acts as a bridge: it accepts the generator factory from the module wrapper,
 * then internally unpacks the generator using recursive `Promise.resolve().then(...)` calls.
 * This ensures that the generated JavaScript runs cleanly in ES5 environments without requiring
 * any changes to the module execution infrastructure.
 */
contextPrototype.h = function (fn: any) {
  return function (handle: any, result: any) {
    var it = fn(handle, result)
    function step(key: any, arg?: any) {
      try {
        var info = it[key](arg)
        var value = info.value
      } catch (error) {
        result(error)
        return
      }
      if (info.done) {
        return
      } else {
        return Promise.resolve(value).then(
          function (value: any) {
            step('next', value)
          },
          function (err: any) {
            step('throw', err)
          }
        )
      }
    }
    return step('next')
  }
}
