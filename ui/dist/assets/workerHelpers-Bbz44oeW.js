let a;
let __tla = (async ()=>{
    function o(r, t) {
        return new Promise((e)=>{
            r.addEventListener("message", function s({ data: n }) {
                n?.type === t && (r.removeEventListener("message", s), e(n));
            });
        });
    }
    o(self, "wasm_bindgen_worker_init").then(async ({ init: r, receiver: t })=>{
        const e = await import("./gtfs_wasm-COB97KvI.js");
        await e.default(r), postMessage({
            type: "wasm_bindgen_worker_ready"
        }), e.wbg_rayon_start_worker(t);
    });
    a = async function(r, t, e) {
        if (e.numThreads() === 0) throw new Error("num_threads must be > 0.");
        const s = {
            type: "wasm_bindgen_worker_init",
            init: {
                module_or_path: r,
                memory: t
            },
            receiver: e.receiver()
        };
        await Promise.all(Array.from({
            length: e.numThreads()
        }, async ()=>{
            const n = new Worker(self.location.href, {
                type: "module"
            });
            return n.postMessage(s), await o(n, "wasm_bindgen_worker_ready"), n;
        })), e.build();
    };
})();
export { a as s, __tla };
