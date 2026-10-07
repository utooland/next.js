'use turbopack: no side effects'

/**
 * `require.cache` compatibility shim.
 *
 * The runtime stores instantiated modules in a `Map` (see `ModuleCache` in
 * `runtime-types.d.ts`), but `require.cache` is specified as an object. Use a
 * Proxy where available, or an object backing store on older targets.
 */

// Access the module cache through the magic context param
// Technically it is also available as a global, but it would be difficult to name.
declare const __turbopack_context__: { c: Map<ModuleId, unknown>; m: Module }
const moduleCache = __turbopack_context__.c

/**
 * Property keys always reach a `Proxy` trap as strings, but `ModuleId` is
 * `string | number` and the id kind is fixed for a build: either every id is a
 * number or every id is a string. Detect this by testing our own id.
 */
const idsAreNumeric = typeof __turbopack_context__.m.id === 'number'

// Coerce a string object property to a ModuleId so it can match the cache.
function toKey(key: string): ModuleId {
  if (!idsAreNumeric) return key
  // A non-canonical spelling (`"0123"`, `"1e3"`, `" 1"`) is not any module's
  // id, so leave it as a string: it will simply miss, as it would have against
  // the object cache.  Without this "1x" would coerce to 1 which might match a module, incorrectly
  const numeric = Number(key)
  return String(numeric) === key ? numeric : key
}

function createLegacyCache(): Record<string, unknown> {
  const cacheMap = moduleCache as typeof moduleCache & {
    __turbopack_require_cache__?: Record<string, unknown>
  }
  if (cacheMap.__turbopack_require_cache__ !== undefined) {
    return cacheMap.__turbopack_require_cache__
  }

  // Plain-object writes and deletes cannot be intercepted without Proxy. Make
  // this object authoritative so the runtime observes them on its next lookup.
  const objectCache: Record<string, unknown> = Object.create(null)
  const get = moduleCache.get.bind(moduleCache)
  const set = moduleCache.set.bind(moduleCache)
  const has = moduleCache.has.bind(moduleCache)
  const remove = moduleCache.delete.bind(moduleCache)
  const clear = moduleCache.clear.bind(moduleCache)
  const keys = moduleCache.keys.bind(moduleCache)
  const values = moduleCache.values.bind(moduleCache)
  const entries = moduleCache.entries.bind(moduleCache)
  const forEach = moduleCache.forEach.bind(moduleCache)
  const getSize = Object.getOwnPropertyDescriptor(Map.prototype, 'size')!.get!

  // A build uses one id kind. Retain Map identity for any other keys, rather
  // than allowing e.g. a string "1" to overwrite the numeric module id 1.
  function isObjectKey(id: ModuleId): boolean {
    return toKey(String(id)) === id
  }

  forEach((value, id) => {
    if (isObjectKey(id)) objectCache[String(id)] = value
  })
  Object.defineProperty(cacheMap, '__turbopack_require_cache__', {
    value: objectCache,
  })

  const hasOwn = Object.prototype.hasOwnProperty
  function synchronize() {
    forEach((_value, id) => {
      if (isObjectKey(id) && !hasOwn.call(objectCache, String(id))) remove(id)
    })
    for (const key of Object.getOwnPropertyNames(objectCache)) {
      set(toKey(key), objectCache[key])
    }
  }

  function liveIterator<T extends Iterator<unknown>>(iterator: T): T {
    const next = iterator.next.bind(iterator)
    iterator.next = () => {
      synchronize()
      return next()
    }
    return iterator
  }

  moduleCache.get = (id) =>
    isObjectKey(id) ? objectCache[String(id)] : get(id)
  moduleCache.has = (id) =>
    isObjectKey(id) ? hasOwn.call(objectCache, String(id)) : has(id)
  moduleCache.set = (id, value) => {
    if (isObjectKey(id)) {
      if (!hasOwn.call(objectCache, String(id))) remove(id)
      objectCache[String(id)] = value
    }
    set(id, value)
    return moduleCache
  }
  moduleCache.delete = (id) => {
    if (!isObjectKey(id)) return remove(id)
    const existed = hasOwn.call(objectCache, String(id))
    delete objectCache[String(id)]
    remove(id)
    return existed
  }
  moduleCache.clear = () => {
    for (const key of Object.getOwnPropertyNames(objectCache)) {
      delete objectCache[key]
    }
    clear()
  }
  moduleCache.keys = () => liveIterator(keys())
  moduleCache.values = () => liveIterator(values())
  moduleCache.entries = () => liveIterator(entries())
  if (typeof Symbol !== 'undefined' && Symbol.iterator) {
    moduleCache[Symbol.iterator] = moduleCache.entries
  }
  moduleCache.forEach = (callback, thisArg) => {
    const iterator = moduleCache.entries()
    for (let entry = iterator.next(); !entry.done; entry = iterator.next()) {
      callback.call(thisArg, entry.value[1], entry.value[0], moduleCache)
    }
  }
  Object.defineProperty(moduleCache, 'size', {
    get() {
      synchronize()
      return getSize.call(moduleCache)
    },
  })

  return objectCache
}

export const cache: Record<string, unknown> =
  typeof Proxy === 'function'
    ? new Proxy(
        // Use an empty object as the 'target' so we can implement ownKeys
        {} as Record<string, unknown>,
        {
          get(_target, key) {
            if (typeof key !== 'string') return undefined
            return moduleCache.get(toKey(key))
          },
          set(_target, key, value) {
            if (typeof key !== 'string') return false
            moduleCache.set(toKey(key), value)
            return true
          },
          has(_target, key) {
            return typeof key === 'string' && moduleCache.has(toKey(key))
          },
          deleteProperty(_target, key) {
            if (typeof key !== 'string') return false
            moduleCache.delete(toKey(key))
            return true
          },
          ownKeys() {
            return Array.from(moduleCache.keys(), String)
          },
          // `ownKeys` alone is not enough: `Object.keys()` filters by enumerability, so
          // it invokes this trap for every key returned above.
          getOwnPropertyDescriptor(_target, key) {
            if (typeof key !== 'string') return undefined
            const id = toKey(key)
            if (!moduleCache.has(id)) return undefined
            return {
              value: moduleCache.get(id),
              writable: true,
              enumerable: true,
              configurable: true,
            }
          },
        }
      )
    : createLegacyCache()
