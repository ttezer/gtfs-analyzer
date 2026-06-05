(async ()=>{
    let u = !1, o = null, y = !1;
    async function w(r = !1) {
        if (u) return;
        const e = globalThis.crossOriginIsolated === !0, a = !r && typeof SharedArrayBuffer < "u" && e;
        if (a) try {
            const t = await import("./gtfs_wasm-BtBSaC6m.js");
            await t.default();
            const n = navigator.hardwareConcurrency || 4, s = Math.max(1, Math.min(n, 8));
            await t.initThreadPool(s), o = t, y = !0, console.info(`[WASM] threaded mode (${s} iş parçacığı)`);
        } catch (t) {
            console.warn("[WASM] threaded init başarısız → seri fallback", t), o = null;
        }
        if (!o) {
            const t = await import("./gtfs_wasm-DwjpF_qU.js");
            await t.default(), o = t, y = !1, console.info(a ? "[WASM] seri mod (threaded yüklenemedi)" : "[WASM] seri mod (cross-origin isolated değil)");
        }
        u = !0;
    }
    function d() {
        if (!o) throw new Error("WASM başlatılmadı (önce initWasm çağrılmalı).");
        return o;
    }
    function b(r) {
        try {
            return JSON.parse(d().list_zip_files(r));
        } catch (e) {
            return console.error("[WASM] listZipFiles başarısız:", e), [];
        }
    }
    function S(r) {
        try {
            return JSON.parse(d().get_cached_file_stats(r));
        } catch  {
            return [];
        }
    }
    function W(r, e) {
        const a = (t, n)=>e(t, n);
        try {
            return d().prepare(r, a);
        } catch (t) {
            const n = typeof t == "string" ? t : String(t), s = F(n);
            throw s && "Fatal" in s ? s.Fatal : {
                code: "ZipUnreadable",
                message: /unreachable|RuntimeError|out of memory|memory access/i.test(n) ? `Feed tarayıcı için çok büyük — WASM bellek yetersiz. Daha küçük bir feed deneyin. (${n})` : n
            };
        }
    }
    function p(r, e = "", a = ()=>{}) {
        const t = (n, s)=>a(n, s);
        return JSON.parse(d().rerun_k6_k7(r, e, t));
    }
    function M(r) {
        return "Ok" in r ? r.Ok : null;
    }
    function A(r) {
        return "Fatal" in r ? r.Fatal : null;
    }
    function F(r) {
        try {
            return JSON.parse(r);
        } catch  {
            return null;
        }
    }
    let c = null;
    self.onmessage = async (r)=>{
        const e = r.data;
        try {
            if (await w(e.type === "validate" ? e.forceSerial ?? !1 : !1), e.type === "validate") {
                const t = new Uint8Array(e.buffer), n = b(t);
                i({
                    id: e.id,
                    type: "file-list",
                    files: n
                });
                const s = (l, k)=>{
                    i({
                        id: e.id,
                        type: "stage",
                        stage: l,
                        elapsed_ms: k
                    });
                };
                c = W(t, s);
                const f = S(c);
                for (const l of f)i({
                    id: e.id,
                    type: "file-done",
                    name: l.name,
                    rows: l.rows,
                    bytes: l.bytes
                });
                const g = p(c, e.configDelta, s);
                h(e.id, g);
                return;
            }
            if (!c) {
                i({
                    id: e.id,
                    type: "result",
                    ok: !1,
                    error: {
                        code: "InvalidInput",
                        message: "Hazır cache yok. ZIP dosyasını yeniden yükleyin."
                    }
                });
                return;
            }
            const a = (t, n)=>{
                i({
                    id: e.id,
                    type: "stage",
                    stage: t,
                    elapsed_ms: n
                });
            };
            h(e.id, p(c, e.configDelta, a));
        } catch (a) {
            if (a !== null && typeof a == "object" && "code" in a) {
                i({
                    id: e.id,
                    type: "result",
                    ok: !1,
                    error: a
                });
                return;
            }
            const t = a instanceof Error ? a.message : String(a);
            i({
                id: e.id,
                type: "result",
                ok: !1,
                error: {
                    code: "ZipUnreadable",
                    message: t
                }
            });
        }
    };
    function i(r) {
        postMessage(r);
    }
    function h(r, e) {
        const a = A(e);
        if (a) {
            i({
                id: r,
                type: "result",
                ok: !1,
                error: a
            });
            return;
        }
        const t = M(e);
        if (!t) {
            i({
                id: r,
                type: "result",
                ok: !1,
                error: {
                    code: "InvalidInput",
                    message: "Beklenmedik sonuç biçimi."
                }
            });
            return;
        }
        i({
            id: r,
            type: "result",
            ok: !0,
            result: t
        });
    }
})();
