import { test } from './util.js'
import { topValue } from './async_module.js'
import './side-effect.js'

// This module has top-level await via its async_module dependency,
// which triggers Turbopack's async module wrapper.
// The wrapper should use function* + __turbopack_context__.h() when
// targeting environments without native async support (e.g. chrome 41).
const value = test()
console.log(value)
console.log(topValue)
