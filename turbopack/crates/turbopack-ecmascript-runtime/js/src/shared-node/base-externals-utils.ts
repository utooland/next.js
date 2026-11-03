/// <reference path="../shared/runtime/runtime-utils.ts" />

/// A 'base' utilities to support runtime can have externals.
/// Currently this is for node.js / edge runtime both.
/// If a fn requires node.js specific behavior, it should be placed in `node-external-utils` instead.

async function externalImport(id: DependencySpecifier) {
  let raw
  try {
    raw = await import(id)
  } catch (err) {
    // TODO(alexkirsz) This can happen when a client-side module tries to load
    // an external module we don't provide a shim for (e.g. querystring, url).
    // For now, we fail semi-silently, but in the future this should be a
    // compilation error.
    throw new Error(`Failed to load external module ${id}: ${err}`)
  }

  if (raw && raw.__esModule && raw.default && 'default' in raw.default) {
    return interopEsm(raw.default, createNS(raw), true)
  }

  return raw
}
contextPrototype.y = externalImport

function externalRequire(
  id: ModuleId,
  thunk: () => any,
  esm: boolean = false
): Exports | EsmNamespaceObject {
  let raw
  try {
    raw = thunk()
  } catch (err) {
    // TODO(alexkirsz) This can happen when a client-side module tries to load
    // an external module we don't provide a shim for (e.g. querystring, url).
    // For now, we fail semi-silently, but in the future this should be a
    // compilation error.
    throw new Error(`Failed to load external module ${id}: ${err}`)
  }

  if (!esm || raw.__esModule) {
    return raw
  }

  return interopEsm(raw, createNS(raw), true)
}

externalRequire.resolve = (
  id: string,
  options?: {
    paths?: string[]
  }
) => {
  return require.resolve(id, options)
}
contextPrototype.x = externalRequire

/**
 * Adds Webpack-compatible ESM metadata to external values while preserving
 * native ESM live bindings.
 */
function externalNamespace(mod: any) {
  if (mod && mod.__esModule) return mod

  const ns = Object.create(null)
  const isEsmNamespace = mod && toStringTag && mod[toStringTag] === 'Module'

  if (mod && (typeof mod === 'object' || typeof mod === 'function')) {
    for (const key in mod) {
      if (key === '__esModule' || (!isEsmNamespace && key === 'default')) {
        continue
      }

      Object.defineProperty(ns, key, {
        enumerable: true,
        get: createGetter(mod, key),
      })
    }
  }

  if (!isEsmNamespace) {
    Object.defineProperty(ns, 'default', { enumerable: true, value: mod })
  }
  Object.defineProperty(ns, '__esModule', { value: true })
  if (toStringTag) {
    Object.defineProperty(ns, toStringTag, { value: 'Module' })
  }

  return ns
}
contextPrototype.N = externalNamespace
