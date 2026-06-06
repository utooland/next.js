#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub fn available_parallelism() -> usize {
    // use wasm_bindgen::{JsCast, prelude::*};
    // let nav = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("navigator"));
    // if let Ok(nav) = nav {
    //     let hc = js_sys::Reflect::get(&nav, &JsValue::from_str("hardwareConcurrency"));
    //     if let Ok(hc) = hc {
    //         if let Some(n) = hc.as_f64() {
    //             return n as usize;
    //         }
    //     }
    // }

    //TODO: Enable concurrency may cause OPFS error, which can't resolve files under .turbopack/
    1
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub fn available_parallelism() -> usize {
    std::thread::available_parallelism().map_or(1, |v| v.get())
}
