(globalThis["TURBOPACK"] || (globalThis["TURBOPACK"] = [])).push(["output/1do3_crates_turbopack-tests_tests_snapshot_basic_async_promise_compat_input_1zt3g7d._.js",
"[project]/turbopack/crates/turbopack-tests/tests/snapshot/basic/async_promise_compat/input/util.js [test] (ecmascript) <internal part 0>", (function(__turbopack_context__){
"use strict";

__turbopack_context__.s([
    "a",
    ()=>test,
    (new_test)=>test = new_test,
    "test",
    ()=>test
]);
function test() {
    return 42;
}
;
;
}),
"[project]/turbopack/crates/turbopack-tests/tests/snapshot/basic/async_promise_compat/input/async_module.js [test] (ecmascript) <internal part 0>", (function(__turbopack_context__){
"use strict";

return __turbopack_context__.a(__turbopack_context__.h(function*(__turbopack_handle_async_dependencies__, __turbopack_async_result__){ try {
__turbopack_context__.s([
    "a",
    ()=>topValue,
    (new_topValue)=>topValue = new_topValue,
    "topValue",
    ()=>topValue
]);
var topValue = await Promise.resolve('top level async');
;
;
;
__turbopack_async_result__();
} catch(e) { __turbopack_async_result__(e); } }), true);}),
"[project]/turbopack/crates/turbopack-tests/tests/snapshot/basic/async_promise_compat/input/side-effect.js [test] (ecmascript)", (function(__turbopack_context__){
"use strict";

return __turbopack_context__.a(__turbopack_context__.h(function*(__turbopack_handle_async_dependencies__, __turbopack_async_result__){ try {
await Promise.resolve('side effect');
console.log('side effect executed');
__turbopack_async_result__();
} catch(e) { __turbopack_async_result__(e); } }), true);}),
"[project]/turbopack/crates/turbopack-tests/tests/snapshot/basic/async_promise_compat/input/index.js [test] (ecmascript)", (function(__turbopack_context__){
"use strict";

return __turbopack_context__.a(__turbopack_context__.h(function*(__turbopack_handle_async_dependencies__, __turbopack_async_result__){ try {
__turbopack_context__.s([]);
var __TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$util$2e$js__$5b$test$5d$__$28$ecmascript$29$__$3c$internal__part__0$3e$__ = __turbopack_context__.i("[project]/turbopack/crates/turbopack-tests/tests/snapshot/basic/async_promise_compat/input/util.js [test] (ecmascript) <internal part 0>");
var __TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$async_module$2e$js__$5b$test$5d$__$28$ecmascript$29$__$3c$internal__part__0$3e$__ = __turbopack_context__.i("[project]/turbopack/crates/turbopack-tests/tests/snapshot/basic/async_promise_compat/input/async_module.js [test] (ecmascript) <internal part 0>");
var __TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$side$2d$effect$2e$js__$5b$test$5d$__$28$ecmascript$29$__ = __turbopack_context__.i("[project]/turbopack/crates/turbopack-tests/tests/snapshot/basic/async_promise_compat/input/side-effect.js [test] (ecmascript)");
var __turbopack_async_dependencies__ = __turbopack_handle_async_dependencies__([
    __TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$async_module$2e$js__$5b$test$5d$__$28$ecmascript$29$__$3c$internal__part__0$3e$__,
    __TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$side$2d$effect$2e$js__$5b$test$5d$__$28$ecmascript$29$__
]);
[__TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$async_module$2e$js__$5b$test$5d$__$28$ecmascript$29$__$3c$internal__part__0$3e$__, __TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$side$2d$effect$2e$js__$5b$test$5d$__$28$ecmascript$29$__] = __turbopack_async_dependencies__.then ? (await __turbopack_async_dependencies__)() : __turbopack_async_dependencies__;
;
;
;
// This module has top-level await via its async_module dependency,
// which triggers Turbopack's async module wrapper.
// The wrapper should use function* + __turbopack_context__.h() when
// targeting environments without native async support (e.g. chrome 41).
var value = (0, __TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$util$2e$js__$5b$test$5d$__$28$ecmascript$29$__$3c$internal__part__0$3e$__["test"])();
console.log(value);
console.log(__TURBOPACK__imported__module__$5b$project$5d2f$turbopack$2f$crates$2f$turbopack$2d$tests$2f$tests$2f$snapshot$2f$basic$2f$async_promise_compat$2f$input$2f$async_module$2e$js__$5b$test$5d$__$28$ecmascript$29$__$3c$internal__part__0$3e$__["topValue"]);
__turbopack_async_result__();
} catch(e) { __turbopack_async_result__(e); } }), false);}),
]);

//# sourceMappingURL=1do3_crates_turbopack-tests_tests_snapshot_basic_async_promise_compat_input_1zt3g7d._.js.map