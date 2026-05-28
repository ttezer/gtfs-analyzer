(function(){const t=document.createElement("link").relList;if(t&&t.supports&&t.supports("modulepreload"))return;for(const d of document.querySelectorAll('link[rel="modulepreload"]'))o(d);new MutationObserver(d=>{for(const h of d)if(h.type==="childList")for(const p of h.addedNodes)p.tagName==="LINK"&&p.rel==="modulepreload"&&o(p)}).observe(document,{childList:!0,subtree:!0});function s(d){const h={};return d.integrity&&(h.integrity=d.integrity),d.referrerPolicy&&(h.referrerPolicy=d.referrerPolicy),d.crossOrigin==="use-credentials"?h.credentials="include":d.crossOrigin==="anonymous"?h.credentials="omit":h.credentials="same-origin",h}function o(d){if(d.ep)return;d.ep=!0;const h=s(d);fetch(d.href,h)}})();const G={page:"upload",result:null,configDelta:sessionStorage.getItem("gtfs-config-delta")??"",fileName:""};function Z(){return G}function _e(e,t){G.result=e,G.fileName=t,G.page="domain"}function D(e){G.page=e}function de(e){G.configDelta=e,sessionStorage.setItem("gtfs-config-delta",e)}const O={CRITICAL:"KRİTİK",HIGH:"YÜKSEK",MEDIUM:"ORTA",LOW:"DÜŞÜK",INFO:"BİLGİ"},U={SPEC:"GTFS Geçerliliği",INTEROP:"GTFS Uyumluluğu",QUALITY:"GTFS Kalitesi",ANALYTICS:"GTFS Analitiği"},ye={ZipUnreadable:"ZIP dosyası açılamadı",Utf8Critical:"UTF-8 kodlama hatası",NoRequiredFiles:"Zorunlu dosyalar eksik",CsvMalformed:"CSV biçim hatası",ResourceLimit:"Notice limiti aşıldı",InvalidInput:"Geçersiz girdi"},j={CRITICAL:"var(--color-critical)",HIGH:"var(--color-high)",MEDIUM:"var(--color-medium)",LOW:"var(--color-low)",INFO:"var(--color-info)"},K=new Map;let ve=1,V=pe();function pe(){const e=new Worker(new URL(""+new URL("validator-worker-Bzfurpmn.js",import.meta.url).href,import.meta.url),{type:"module"});return e.onmessage=ke,e.onerror=Se,e}function ke(e){const t=e.data,s=K.get(t.id);if(s){if(t.type==="file-list"){s.callbacks?.onFileList?.(t.files);return}if(t.type==="file-done"){s.callbacks?.onFileDone?.(t.name,t.rows,t.bytes);return}if(t.type==="stage"){s.callbacks?.onStageDone?.(t.stage,t.elapsed_ms);return}t.type==="result"&&(K.delete(t.id),t.ok?s.resolve(t.result):s.reject(t.error))}}function Se(e){const t={code:"ResourceLimit",message:e.message||"Arka plan doğrulama işçisi beklenmedik şekilde durdu."};for(const[,s]of K)s.reject(t);K.clear(),V.terminate(),V=pe()}function $e(e,t,s){const o=ve++;return new Promise((d,h)=>{K.set(o,{resolve:d,reject:h,callbacks:s}),V.postMessage({id:o,type:"validate",buffer:e,configDelta:t},[e])})}const ue={K1:"K1 — Dosya Ayrıştırma",K2:"K2 — Kural Doğrulama",K3:"K3 — Varlık Haritası",K4:"K4 — Çapraz Kontrol",K5:"K5 — Türetilmiş Veri",K6:"K6 — İstatistik Analizi",K7:"K7 — Rapor Oluşturma"},C=["K1","K2","K3","K4","K5","K6","K7"],Te=[{key:"max_speed_bus_kmh",label:"Maks. Otobüs Hızı",unit:"km/s",type:"float",def:120,min:60,max:200,desc:"Otobüs seferleri için maksimum izin verilen hız"},{key:"max_speed_tram_kmh",label:"Maks. Tramvay Hızı",unit:"km/s",type:"float",def:100,min:40,max:160,desc:"Tramvay seferleri için maksimum izin verilen hız"},{key:"max_speed_metro_kmh",label:"Maks. Metro Hızı",unit:"km/s",type:"float",def:150,min:80,max:250,desc:"Metro seferleri için maksimum izin verilen hız"},{key:"max_speed_rail_kmh",label:"Maks. Demiryolu Hızı",unit:"km/s",type:"float",def:300,min:100,max:400,desc:"Demiryolu seferleri için maksimum izin verilen hız"},{key:"max_speed_ferry_kmh",label:"Maks. Feribot Hızı",unit:"km/s",type:"float",def:80,min:20,max:150,desc:"Feribot seferleri için maksimum izin verilen hız"},{key:"max_speed_cablecar_kmh",label:"Maks. Teleferik Hızı",unit:"km/s",type:"float",def:30,min:10,max:60,desc:"Teleferik/füniküler için maksimum izin verilen hız"},{key:"min_transfer_time_sec",label:"Min. Aktarma Süresi",unit:"sn",type:"int",def:180,min:30,max:1800,desc:"Transferler için minimum bağlantı süresi"},{key:"max_transfer_distance_m",label:"Maks. Aktarma Mesafesi",unit:"m",type:"float",def:500,min:50,max:2e3,desc:"Transfer geçerli sayılmak için maksimum mesafe"},{key:"max_shape_jump_km",label:"Maks. Güzergah Sıçraması",unit:"km",type:"float",def:10,min:1,max:50,desc:"Art arda güzergah noktaları arasındaki maksimum mesafe"},{key:"stop_too_close_m",label:"Çok Yakın Durak Eşiği",unit:"m",type:"float",def:5,min:1,max:20,desc:"Bu mesafeden yakın duraklar tekrar sayılır"},{key:"stop_far_from_shape_m",label:"Durağın Güzergah'a Uzaklığı",unit:"m",type:"float",def:100,min:20,max:500,desc:"Durağın güzergahından en fazla bu kadar uzakta olabilir"},{key:"stop_far_from_parent_m",label:"Üst İstasyona Uzaklık Eşiği",unit:"m",type:"float",def:100,min:10,max:1e3,desc:"Durak, üst istasyonundan en fazla bu kadar uzakta olabilir"},{key:"feed_expiry_warning_days",label:"Son Kullanma Uyarısı",unit:"gün",type:"int",def:30,min:1,max:60,desc:"Feed bu kadar günden az kalmışsa uyarı üretilir"},{key:"service_gap_days",label:"Servis Boşluğu Eşiği",unit:"gün",type:"int",def:7,min:3,max:30,desc:"Bu günden uzun servis kesintisi işaretlenir"},{key:"max_trip_duration_hours",label:"Maks. Sefer Süresi",unit:"saat",type:"float",def:24,min:8,max:72,desc:"Tek bir seferin maksimum süresi"},{key:"min_trip_duration_sec",label:"Min. Sefer Süresi",unit:"sn",type:"int",def:60,min:10,max:300,desc:"Tek bir seferin minimum süresi"},{key:"max_headway_warning_min",label:"Maks. Sefer Aralığı",unit:"dk",type:"int",def:240,min:60,max:720,desc:"Bu dakikadan uzun aralık uyarı üretir"},{key:"bunching_threshold_min",label:"Sıkışma Eşiği",unit:"dk",type:"int",def:2,min:1,max:10,desc:"Bu dakikadan kısa aralık sıkışma sayılır"}],fe={"stops.txt":"durak","routes.txt":"hat","trips.txt":"sefer","stop_times.txt":"geçiş zamanı","shapes.txt":"güzergah","agency.txt":"işletici","calendar.txt":"takvim","calendar_dates.txt":"özel gün","frequencies.txt":"frekans","transfers.txt":"aktarma","pathways.txt":"geçit","levels.txt":"kat","attributions.txt":"atıf","translations.txt":"çeviri"};function we(e){const t=Z(),s=t.result!==null,o=s?"Farklı GTFS Yükle":"GTFS Yükle";e.innerHTML=`
    <div class="up-grid">

      <!-- Sol üst: GTFS Dosyaları -->
      <div class="up-cell up-files">
        <div class="lp-section-title">GTFS Dosyaları</div>
        <div id="file-rows" class="lp-rows">
          ${s&&t.result.metrics.file_stats.length>0?he(t.result.metrics.file_stats):'<div class="lp-placeholder">Henüz dosya yüklenmedi</div>'}
        </div>
      </div>

      <!-- Sağ üst: ZIP Yükle -->
      <div class="up-cell up-drop">
        <div id="drop-zone" class="drop-zone" role="button" tabindex="0" aria-label="ZIP dosyası yükle">
          <svg class="upload-icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5"/>
          </svg>
          <span class="drop-primary">ZIP sürükleyin</span>
          <label id="upload-btn-label" class="btn btn-primary btn-sm" for="file-input">${P(o)}</label>
          <input type="file" id="file-input" accept=".zip" class="visually-hidden" aria-label="ZIP dosyası seç"/>
        </div>
        <div id="upload-error" class="upload-status hidden"></div>
      </div>

      <!-- Sol alt: İşlem Aşamaları -->
      <div class="up-cell up-stages">
        <div class="lp-section-title">İşlem Aşamaları</div>
        <div id="stage-rows" class="lp-rows">
          ${C.map(d=>`
            <div class="lp-stage-row ${s?"done":"waiting"}" data-stage="${d}">
              <span class="lp-icon">${s?"✓":"○"}</span>
              <span class="lp-label">${P(ue[d]??d)}</span>
              <span class="lp-status">${s?"":"—"}</span>
            </div>`).join("")}
        </div>
      </div>

      <!-- Sağ alt: Kalite Skoru + Rapor butonu -->
      <div class="up-cell up-score">
        <div id="score-panel" class="score-compact${s?"":" hidden"}">
          ${s?ge(t.result):""}
        </div>
        <button id="btn-report" class="btn btn-primary btn-report" ${s?"":"disabled"}>
          Raporu Göster →
        </button>
      </div>

    </div>

    <!-- Analiz Kriterleri (collapse) -->
    <div class="settings-wrap">
      <button class="settings-toggle" id="settings-toggle" type="button">
        <span class="settings-arrow" id="settings-arrow">▶</span>
        Analiz Kriterleri
        ${t.configDelta&&t.configDelta!=="{}"?'<span class="settings-badge">Değiştirildi</span>':""}
      </button>
      <div class="settings-body hidden" id="settings-body">
        <div class="settings-grid" id="settings-grid">
          ${Le(ze(t.configDelta))}
        </div>
        <div class="settings-actions">
          <button id="btn-apply-settings" class="btn btn-primary btn-sm">Uygula</button>
          <button id="btn-reset-settings" class="btn btn-ghost btn-sm">Sıfırla</button>
          <span id="settings-status" class="settings-status hidden"></span>
        </div>
      </div>
    </div>`,Re(e)}function he(e){return e.map(t=>{const s=Math.round(t.bytes/1024),o=fe[t.name]??"satır";return`
      <div class="lp-file-row done">
        <span class="lp-icon">✓</span>
        <span class="lp-label">${P(t.name)}</span>
        <span class="lp-status">${t.rows.toLocaleString("tr-TR")} ${o} · ${W(s)}</span>
      </div>`}).join("")}function ge(e){const{r5:t}=e.reports,s=[{label:"GTFS Geçerliliği",weight:"×40%",value:t.spec_score,color:"#2563eb",bg:"#eff6ff"},{label:"GTFS Uyumluluğu",weight:"×30%",value:t.interop_score,color:"#92400e",bg:"#fef3c7"},{label:"GTFS Kalitesi",weight:"×20%",value:t.quality_score,color:"#15803d",bg:"#f0fdf4"},{label:"GTFS Analitiği",weight:"×10%",value:t.analytics_score,color:"#6d28d9",bg:"#f5f3ff"}],o=t.score>=80?"#15803d":t.score>=60?"#92400e":"#dc2626",d=t.pub_score>=80?"#15803d":t.pub_score>=60?"#d97706":"#dc2626",h=s.map(p=>`<div class="sc-chip" style="background:${p.bg};color:${p.color}">
      <div class="sc-chip-left">
        <span class="sc-chip-label">${p.label}</span>
        <span class="sc-weight">${p.weight}</span>
      </div>
      <span class="sc-chip-val">${p.value.toFixed(1)}</span>
    </div>`).join("");return`
    <div class="sc-dual-header">
      <div class="sc-dual-item">
        <span class="sc-title">Yayın Skoru</span>
        <span class="sc-overall" style="color:${d}">${t.pub_score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
      <div class="sc-dual-sep"></div>
      <div class="sc-dual-item">
        <span class="sc-title">Kalite Skoru</span>
        <span class="sc-overall" style="color:${o}">${t.score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
    </div>
    <div class="sc-chips">${h}</div>`}function Le(e){return Te.map(t=>{const s=e[t.key]!==void 0?e[t.key]:t.def,o=e[t.key]!==void 0;return`
      <div class="sf-row${o?" sf-overridden":""}">
        <label class="sf-label" title="${P(t.desc)}">${P(t.label)}</label>
        <div class="sf-control">
          <input class="sf-input" type="number" data-key="${t.key}" data-def="${t.def}"
            data-type="${t.type}" min="${t.min}" max="${t.max}"
            step="${t.type==="int"?1:.1}" value="${s}"/>
          <span class="sf-unit">${t.unit}</span>
          <button class="sf-reset btn btn-ghost" data-key="${t.key}" title="Sıfırla"
            ${o?"":'style="visibility:hidden"'}>✕</button>
        </div>
        <div class="sf-desc">${P(t.desc)} (varsayılan: ${t.def})</div>
      </div>`}).join("")}function ze(e){if(!e||e==="{}")return{};try{return JSON.parse(e)}catch{return{}}}function Re(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),o=e.querySelector("#upload-error");s.addEventListener("change",()=>{const u=s.files?.[0];u&&se(u,e,o)}),t.addEventListener("dragover",u=>{u.preventDefault(),t.classList.contains("loading")||t.classList.add("drag-over")}),t.addEventListener("dragleave",()=>t.classList.remove("drag-over")),t.addEventListener("drop",u=>{if(u.preventDefault(),t.classList.remove("drag-over"),t.classList.contains("loading"))return;const a=u.dataTransfer?.files[0];a&&se(a,e,o)}),t.addEventListener("keydown",u=>{(u.key==="Enter"||u.key===" ")&&!t.classList.contains("loading")&&s.click()}),e.querySelector("#btn-report")?.addEventListener("click",()=>{D("domain"),H()});const d=e.querySelector("#settings-toggle"),h=e.querySelector("#settings-body"),p=e.querySelector("#settings-arrow");d.addEventListener("click",()=>{const u=!h.classList.contains("hidden");h.classList.toggle("hidden",u),p.textContent=u?"▶":"▼"}),e.querySelector("#btn-apply-settings")?.addEventListener("click",()=>Pe(e)),e.querySelector("#btn-reset-settings")?.addEventListener("click",()=>Me(e)),e.querySelectorAll(".sf-reset").forEach(u=>{u.addEventListener("click",()=>{const a=u.dataset.key,l=e.querySelector(`.sf-input[data-key="${CSS.escape(a)}"]`);l&&(l.value=l.dataset.def,l.closest(".sf-row")?.classList.remove("sf-overridden"),u.style.visibility="hidden")})}),e.querySelectorAll(".sf-input").forEach(u=>{u.addEventListener("change",()=>{const a=e.querySelector(`.sf-reset[data-key="${CSS.escape(u.dataset.key)}"]`);a&&(a.style.visibility="visible"),u.closest(".sf-row")?.classList.add("sf-overridden")})})}function Pe(e){const t={};e.querySelectorAll(".sf-input").forEach(d=>{const h=d.dataset.key,p=parseFloat(d.dataset.def),u=d.dataset.type==="int"?parseInt(d.value,10):parseFloat(d.value);!isNaN(u)&&u!==p&&(t[h]=u)});const s=Object.keys(t).length>0?JSON.stringify(t):"";de(s);const o=e.querySelector("#settings-status");o.textContent="Ayarlar kaydedildi. Yeni ZIP yükleyince uygulanır.",o.className="settings-status ok",setTimeout(()=>{o.className="settings-status hidden"},3e3)}function Me(e){e.querySelectorAll(".sf-input").forEach(s=>{s.value=s.dataset.def,s.closest(".sf-row")?.classList.remove("sf-overridden");const o=e.querySelector(`.sf-reset[data-key="${CSS.escape(s.dataset.key)}"]`);o&&(o.style.visibility="hidden")}),de("");const t=e.querySelector("#settings-status");t.textContent="Varsayılanlara döndürüldü.",t.className="settings-status ok",setTimeout(()=>{t.className="settings-status hidden"},3e3)}async function se(e,t,s){if(!e.name.endsWith(".zip")){le(s,{code:"InvalidInput",message:"Yalnızca .zip uzantılı dosyalar kabul edilir."});return}s.className="upload-status hidden",Ae(t,e.name,e.size);try{const o=await e.arrayBuffer(),d=await $e(o,Z().configDelta,{onFileList:h=>{const p=t.querySelector("#file-rows");p&&(p.innerHTML="",h.forEach(u=>{const a=document.createElement("div");a.className="lp-file-row reading",a.dataset.file=u.name;const l=Math.round(u.uncompressed_size/1024);a.innerHTML=`
            <span class="lp-icon">○</span>
            <span class="lp-label">${P(u.name)}</span>
            <span class="lp-status">${W(l)} okunuyor…</span>`,p.appendChild(a)}))},onFileDone:(h,p,u)=>{const l=t.querySelector("#file-rows")?.querySelector(`[data-file="${CSS.escape(h)}"]`);if(!l)return;l.className="lp-file-row done",l.querySelector(".lp-icon").textContent="✓";const r=fe[h]??"satır";l.querySelector(".lp-status").textContent=`${p.toLocaleString("tr-TR")} ${r} · ${W(Math.round(u/1024))}`},onStageDone:(h,p)=>{const u=t.querySelector(`[data-stage="${CSS.escape(h)}"]`);if(!u)return;u.className="lp-stage-row done",u.querySelector(".lp-icon").textContent="✓",u.querySelector(".lp-status").textContent=p<1e3?`${p} ms`:`${(p/1e3).toFixed(1)} sn`;const a=C.indexOf(h);if(a>=0&&a<C.length-1){const l=t.querySelector(`[data-stage="${CSS.escape(C[a+1])}"]`);l?.classList.contains("waiting")&&(l.className="lp-stage-row running",l.querySelector(".lp-icon").textContent="⋯",l.querySelector(".lp-status").textContent="çalışıyor…")}}});_e(d,e.name),Ee(t,d)}catch(o){be(t),le(s,o)}}function Ae(e,t,s){const o=e.querySelector("#drop-zone"),d=e.querySelector("#file-input"),h=e.querySelector("#upload-btn-label"),p=e.querySelector("#score-panel");p&&(p.classList.add("hidden"),p.innerHTML="");const u=e.querySelector("#btn-report");u&&(u.disabled=!0),o.classList.add("loading"),d.disabled=!0,h.setAttribute("aria-disabled","true"),h.style.opacity="0.5",h.style.pointerEvents="none";const a=(s/1048576).toFixed(1),l=o.querySelector(".drop-primary");l&&(l.textContent=`${t} (${a} MB) işleniyor…`);const r=e.querySelector("#stage-rows");r&&(r.innerHTML=C.map((n,i)=>`
      <div class="lp-stage-row ${i===0?"running":"waiting"}" data-stage="${n}">
        <span class="lp-icon">${i===0?"⋯":"○"}</span>
        <span class="lp-label">${P(ue[n]??n)}</span>
        <span class="lp-status">${i===0?"çalışıyor…":"bekliyor"}</span>
      </div>`).join(""))}function be(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),o=e.querySelector("#upload-btn-label");t.classList.remove("loading"),s.disabled=!1,o.removeAttribute("aria-disabled"),o.style.opacity="",o.style.pointerEvents=""}function Ee(e,t){be(e);const s=e.querySelector("#upload-btn-label");s.textContent="Farklı GTFS Yükle";const o=e.querySelector("#score-panel");o.className="score-compact",o.innerHTML=ge(t);const d=e.querySelector("#btn-report");d.disabled=!1,d.addEventListener("click",()=>{D("domain"),H()},{once:!0});const h=e.querySelector("#file-rows");h&&t.metrics.file_stats.length>0&&(h.innerHTML=he(t.metrics.file_stats))}function le(e,t){const s=ye[t.code]??t.code;e.className="upload-status error",e.innerHTML=`<strong>${P(s)}</strong><p>${P(t.message)}</p>`}function W(e){return e>=1024?`${(e/1024).toFixed(1)} MB`:`${e} KB`}function P(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function Fe(e,t){const{r1:s,r5:o}=t.reports,d=t.metrics,h=De(t);e.innerHTML=`
    <div class="report-page">
      ${Ge(o,h)}
      ${He(s,t)}
      ${Oe(o)}
      ${Ce(d)}
    </div>`}function Ge(e,t){const s=ae(e.pub_score),o=ae(e.score);return`
    <div class="rpt-score-row">
      <div class="rpt-score-card">
        <span class="rpt-score-label">Yayın Skoru</span>
        <span class="rpt-score-val" style="color:${s}">${e.pub_score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${e.pub_score.toFixed(1)}%;background:${s}"></div></div>
        <span class="rpt-score-hint">SPEC + INTEROP blocker temizliği</span>
      </div>
      <div class="rpt-score-card">
        <span class="rpt-score-label">Kalite Skoru</span>
        <span class="rpt-score-val" style="color:${o}">${e.score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${e.score.toFixed(1)}%;background:${o}"></div></div>
        <span class="rpt-score-hint">Tüm sınıfların ağırlıklı ortalaması</span>
      </div>
      ${qe(t)}
    </div>`}function ae(e){return e>=80?"#16a34a":e>=60?"#d97706":"#dc2626"}function He(e,t){const s=new Map(t.notices.map(o=>[o.id,o]));if(!e.publishable){const o=e.blocker_notice_ids.slice(0,5).map(d=>{const h=s.get(d);return h?`<li><code>${q(h.rule_id)}</code> — ${q(h.title||h.message.slice(0,80))}</li>`:""}).join("");return`
      <div class="rpt-r1 rpt-r1-blocked">
        <div class="rpt-r1-icon">✕</div>
        <div class="rpt-r1-body">
          <strong>Yayınlanması Tavsiye Edilmez</strong>
          <span>${e.blocker_notice_ids.length} kritik engelleyici bulgu mevcut — feed yayına alınamaz.</span>
          ${o?`<ul class="rpt-r1-list">${o}</ul>`:""}
        </div>
      </div>`}if(e.conditional){const o=e.conditional_blocker_notice_ids.slice(0,5).map(d=>{const h=s.get(d);return h?`<li><code>${q(h.rule_id)}</code> — ${q(h.title||h.message.slice(0,80))}</li>`:""}).join("");return`
      <div class="rpt-r1 rpt-r1-conditional">
        <div class="rpt-r1-icon">⚠</div>
        <div class="rpt-r1-body">
          <strong>Koşullu Yayına Uygun</strong>
          <span>${e.conditional_blocker_notice_ids.length} koşullu engelleyici mevcut — bazı uygulamalar bu feed'i reddedebilir.</span>
          ${o?`<ul class="rpt-r1-list">${o}</ul>`:""}
        </div>
      </div>`}return`
    <div class="rpt-r1 rpt-r1-ok">
      <div class="rpt-r1-icon">✓</div>
      <div class="rpt-r1-body">
        <strong>Yayına Uygun</strong>
        <span>Engelleyici bulgu yok.</span>
      </div>
    </div>`}function Oe(e){return`
    <div>
      <div class="rpt-section-title" style="margin-bottom:.4rem">Kalite Skoru Bileşenleri</div>
      <div class="rpt-sub-row">${[{label:"GTFS Geçerliliği",weight:"%40",value:e.spec_score,color:"#1d4ed8",bg:"#eff6ff"},{label:"GTFS Uyumluluğu",weight:"%30",value:e.interop_score,color:"#b45309",bg:"#fef3c7"},{label:"GTFS Kalitesi",weight:"%20",value:e.quality_score,color:"#15803d",bg:"#f0fdf4"},{label:"GTFS Analitiği",weight:"%10",value:e.analytics_score,color:"#6d28d9",bg:"#f5f3ff"}].map(o=>`
    <div class="rpt-sub-chip" style="background:${o.bg};color:${o.color}">
      <div class="rpt-sub-top">
        <span class="rpt-sub-label">${o.label}</span>
        <span class="rpt-sub-weight">${o.weight}</span>
      </div>
      <span class="rpt-sub-val">${o.value.toFixed(1)}</span>
    </div>`).join("")}</div>
    </div>`}const oe=[{key:"CRITICAL",label:"Kritik",color:"#dc2626"},{key:"HIGH",label:"Yüksek",color:"#d97706"},{key:"MEDIUM",label:"Orta",color:"#2563eb"},{key:"LOW",label:"Düşük",color:"#16a34a"},{key:"INFO",label:"Bilgi",color:"#9ca3af"}];function qe(e){const t=e.CRITICAL+e.HIGH+e.MEDIUM+e.LOW+e.INFO;if(t===0)return"";const s=15.9155;let o=0;const d=oe.filter(p=>e[p.key]>0).map(p=>{const u=e[p.key]/t*100,a=25-o;return o+=u,`<circle cx="18" cy="18" r="${s}" fill="none" stroke="${p.color}" stroke-width="3.5" stroke-dasharray="${u.toFixed(3)} ${(100-u).toFixed(3)}" stroke-dashoffset="${a.toFixed(3)}"/>`}).join(""),h=oe.filter(p=>e[p.key]>0).map(p=>{const u=e[p.key],a=(u/t*100).toFixed(1);return`<div class="pie-leg-row"><span class="pie-leg-dot" style="background:${p.color}"></span><span class="pie-leg-label">${p.label}</span><span class="pie-leg-count">${u.toLocaleString("tr-TR")}</span><span class="pie-leg-pct">${a}%</span></div>`}).join("");return`
    <div class="rpt-score-card rpt-pie-card">
      <span class="rpt-score-label">Hata Dağılımı <span class="rpt-score-total">${t.toLocaleString("tr-TR")}</span></span>
      <div class="rpt-pie-body">
        <svg viewBox="0 0 36 36" class="rpt-pie-svg" aria-hidden="true">
          <circle cx="18" cy="18" r="${s}" fill="none" class="pie-track" stroke-width="3.5"/>
          ${d}
          <text x="18" y="16" text-anchor="middle" dominant-baseline="middle"
            font-size="5.5" font-weight="800" class="pie-center-num">${t.toLocaleString("tr-TR")}</text>
          <text x="18" y="22" text-anchor="middle" dominant-baseline="middle"
            font-size="2.8" class="pie-center-txt">hata</text>
        </svg>
        <div class="rpt-pie-legend">${h}</div>
      </div>
    </div>`}function De(e){const t={CRITICAL:0,HIGH:0,MEDIUM:0,LOW:0,INFO:0};for(const s of e.notices)s.severity in t&&t[s.severity]++;return t}function Ce(e){const s=[{label:"Durak",value:e.stop_count.toLocaleString("tr-TR")},{label:"Hat",value:e.route_count.toLocaleString("tr-TR")},{label:"Sefer",value:e.trip_count.toLocaleString("tr-TR")},{label:"Güzergah",value:e.shape_count.toLocaleString("tr-TR")},{label:"Aktif Servis Günü",value:e.active_service_days.toLocaleString("tr-TR")},{label:"Ort. Günlük Sefer",value:e.avg_daily_trips.toFixed(0)}].map(d=>`
    <div class="rpt-metric">
      <span class="rpt-metric-val">${d.value}</span>
      <span class="rpt-metric-label">${d.label}</span>
    </div>`).join(""),o=e.file_stats.map(d=>`
    <tr>
      <td><code>${q(d.name)}</code></td>
      <td class="num">${d.rows.toLocaleString("tr-TR")}</td>
      <td class="num">${Ke(d.bytes)}</td>
    </tr>`).join("");return`
    <div class="card">
      <h3 class="rpt-section-title">Feed Metrikleri</h3>
      <div class="rpt-metrics-grid">${s}</div>
      ${o?`
        <div class="table-scroll rpt-files">
          <table class="data-table">
            <thead><tr><th>Dosya</th><th class="num">Satır</th><th class="num">Boyut</th></tr></thead>
            <tbody>${o}</tbody>
          </table>
        </div>`:""}
    </div>`}function Ke(e){return e<1024?`${e} B`:e<1024*1024?`${(e/1024).toFixed(1)} KB`:`${(e/1024/1024).toFixed(1)} MB`}function q(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}let T=null,$=null;function Be(e,t){Ne(),T.querySelector(".mm-title").textContent=e,T.classList.remove("mm-hidden");const s=T.querySelector(".mm-map");$?$.eachLayer(p=>{p instanceof L.TileLayer||$.removeLayer(p)}):($=L.map(s),L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",{attribution:"© OpenStreetMap katkıcıları",maxZoom:19}).addTo($));const o=[];if(t.polyline&&t.polyline.length>1){L.polyline(t.polyline,{color:"#f59e0b",weight:3,opacity:.8}).addTo($);for(const p of t.polyline)o.push(p);if(t.showArrows)for(const p of xe(t.polyline,250))L.marker([p.lat,p.lon],{icon:L.divIcon({html:`<svg width="14" height="14" viewBox="-7 -7 14 14" style="transform:rotate(${p.angle}deg);display:block">
              <polygon points="0,-6 5,4 -5,4" fill="#f59e0b" stroke="#fff" stroke-width="1.5"/>
            </svg>`,className:"",iconSize:[14,14],iconAnchor:[7,7]}),interactive:!1}).addTo($)}let d=null;if(t.extraPolylines){for(const p of t.extraPolylines)if(p.coords.length>1){L.polyline(p.coords,{color:p.color,weight:p.weight??4,opacity:.95}).addTo($);for(const u of p.coords)o.push(u);p.zoomTo&&(d=p.coords)}}for(const p of t.pins){const u=p.primary?"#2563eb":"#dc2626",a=p.small?5:9,l=p.small?1.5:2.5;L.circleMarker([p.lat,p.lon],{radius:a,fillColor:u,color:"#fff",weight:l,fillOpacity:.85}).addTo($).bindPopup(p.label,{maxWidth:260}),o.push([p.lat,p.lon])}d&&d.length>1?$.fitBounds(d,{padding:[60,60],maxZoom:16}):o.length===0?$.setView([39,35],6):o.length===1?$.setView(o[0],16):$.fitBounds(o,{padding:[40,40]});const h=T.querySelector(".mm-legend");t.legendItems&&t.legendItems.length>0?(h.innerHTML=t.legendItems.map(p=>`<div class="mm-legend-item"><div class="mm-dot" style="background:${p.color}"></div>${p.label}</div>`).join(""),h.style.display="flex"):h.style.display="none",setTimeout(()=>$.invalidateSize(),50)}function Y(){T?.classList.add("mm-hidden")}function xe(e,t){const o=l=>l*Math.PI/180,d=(l,r,n,i)=>{const c=o(n-l),f=o(i-r),g=Math.sin(c/2)**2+Math.cos(o(l))*Math.cos(o(n))*Math.sin(f/2)**2;return 12742e3*Math.atan2(Math.sqrt(g),Math.sqrt(1-g))},h=(l,r,n,i)=>{const c=o(i-r),f=Math.sin(c)*Math.cos(o(n)),g=Math.cos(o(l))*Math.sin(o(n))-Math.sin(o(l))*Math.cos(o(n))*Math.cos(c);return(Math.atan2(f,g)*180/Math.PI+360)%360},p=[];let u=0,a=t*.5;for(let l=0;l<e.length-1;l++){const[r,n]=e[l],[i,c]=e[l+1],f=d(r,n,i,c),g=h(r,n,i,c);for(;u+f>=a;){const b=(a-u)/f;p.push({lat:r+b*(i-r),lon:n+b*(c-n),angle:g}),a+=t}u+=f}return p}function Ne(){T||(T=document.createElement("div"),T.className="mm-overlay mm-hidden",T.innerHTML=`
    <div class="mm-box" role="dialog" aria-modal="true" aria-label="Harita">
      <div class="mm-header">
        <span class="mm-title"></span>
        <button class="mm-close" title="Kapat" aria-label="Kapat">✕</button>
      </div>
      <div class="mm-map"></div>
      <div class="mm-legend"></div>
    </div>`,document.body.appendChild(T),T.querySelector(".mm-close").addEventListener("click",Y),T.addEventListener("click",e=>{e.target===T&&Y()}),document.addEventListener("keydown",e=>{e.key==="Escape"&&Y()}))}const Ie={blocker:"Engelleyici",conditional_blocker:"Koşullu Engelleyici",interop:"Uyumluluk",propagation:"Zincirleme","quick-win":"Kolay Düzeltme",quality:"Kalite Öncelikli",widespread:"Yaygın",analytics:"Analitik",hard:"Zor",single:"Tek Örnek","high-impact":"Yüksek Etki"};function je(e,t){const s=new Map(t.notices.map(a=>[a.id,a])),o=t.reports.r9.items.reduce((a,l)=>a+l.score_delta,0),d=o>0?(100-t.reports.r5.score)/o:1,h=t.reports.r9.items.reduce((a,l)=>a+l.pub_score_delta,0),p=h>0?(100-t.reports.r5.pub_score)/h:1,u=new Map(t.reports.r9.items.map(a=>[a.rule_id,{qualDelta:a.score_delta*d,pubDelta:a.pub_score_delta*p,count:a.affected_instance_count}]));e.innerHTML=`
    <section class="page-fix">
      ${Ue(t.reports.r9.items,s,d,p,t.capped_totals)}
      ${Ye(t,s,u,t.name_index)}
    </section>`}function Ue(e,t,s,o,d){if(e.length===0)return'<div class="card"><h2>Düzeltme Kuyruğu (R9)</h2><p class="empty">Düzeltilecek bulgu yok.</p></div>';const h={};for(const u of t.values())h[u.rule_id]=(h[u.rule_id]??0)+1;const p=e.map((u,a)=>{const l=u.notice_ids[0]?t.get(u.notice_ids[0]):void 0,r=l?.severity??"INFO",n=u.labels.map(S=>`<span class="label-badge label-${S}">${Ie[S]??S}</span>`).join(""),i=u.pub_score_delta*o,c=u.score_delta*s,f=i>=.05?`<span class="pub-delta">+${i.toFixed(1)}</span>`:'<span class="muted-text">—</span>',g=c>=.05?`<span class="score-delta">+${c.toFixed(1)}</span>`:'<span class="muted-text">—</span>',b=d[u.rule_id],m=b!=null?`<span class="cap-total">${b.toLocaleString("tr-TR")}</span>`:'<span class="muted-text">—</span>',_=h[u.rule_id]??u.affected_instance_count,v=`
      <tr class="r9-main-row" data-idx="${a}">
        <td>
          <span class="r9-arrow">▶</span> <code>${w(u.rule_id)}</code>
          ${l?.title?`<span class="r9-rule-title">${w(l.title)}</span>`:""}
        </td>
        <td style="color:${j[r]}">${O[r]}</td>
        <td>${n}</td>
        <td>${_.toLocaleString("tr-TR")}</td>
        <td>${m}</td>
        <td class="score">${u.priority_score.toFixed(1)}</td>
        <td class="score-delta-cell">${f}</td>
        <td class="score-delta-cell">${g}</td>
        <td>${u.realized_dependent_count}</td>
        <td>${u.fix_effort.toFixed(1)}</td>
      </tr>`,k=`
      <tr class="r9-detail-row" data-for="${a}" hidden>
        <td colspan="10">
          <div class="r9-detail">
            ${l?.title?`<p><strong>Mesaj:</strong> ${w(l.title)}</p>`:""}
            ${l?.remediation?`<p><strong>Çözüm:</strong> ${w(l.remediation)}</p>`:""}
          </div>
        </td>
      </tr>`;return v+k}).join("");return`
    <div class="card">
      <h2>Düzeltme Kuyruğu (R9) <span class="count-badge">${e.length}</span></h2>
      <p class="hint">Satıra tıklayarak açıklama ve çözüm önerisini görebilirsiniz.</p>
      <div class="table-scroll">
        <table class="data-table" id="r9-table">
          <thead><tr>
            <th>Kural</th>
            <th>Önem</th>
            <th>Etiket</th>
            <th>Hata <span class="col-info" title="Bu kuraldan kaç hata üretildi — sorunun yaygınlığını gösterir.">ℹ</span></th>
            <th>Toplam <span class="col-info" title="Kural başına notice sınırı (cap) uygulandığında gerçek ihlal sayısı. Raporda yalnızca sınır kadar notice gösterilir; sınıra çarpmayan kurallarda bu sütun boş kalır.">ℹ</span></th>
            <th class="score">Skor <span class="col-info" title="Öncelik puanı. Ciddiyet × (1+Bağımlı) × log₂(1+Etkilenen) / Çaba formülüyle hesaplanır. Yüksek skor = önce düzelt.">ℹ</span></th>
            <th class="score-delta-cell">+Yayın <span class="col-info" title="Bu kural düzeltilirse yayın skoru kaç puan artar. Yalnızca blocker/conditional_blocker kurallar için gösterilir. Toplamı = 100 − mevcut yayın skoru.">ℹ</span></th>
            <th class="score-delta-cell">+Kalite <span class="col-info" title="Bu kural düzeltilirse kalite skoru kaç puan artar. Toplamı = 100 − mevcut kalite skoru.">ℹ</span></th>
            <th>Bağımlı <span class="col-info" title="Bu kural düzeltilince kaç başka kural daha kendiliğinden kapanır (bu feed'de ateşlenmiş olan bağımlı kurallar).">ℹ</span></th>
            <th>Çaba <span class="col-info" title="Düzeltmenin iş yükü: 1=kolay (tek alan değişikliği), 5=zor (veri modelinde kapsamlı revizyon).">ℹ</span></th>
          </tr></thead>
          <tbody>${p}</tbody>
        </table>
      </div>
    </div>`}function Ye(e,t,s,o){const d=e.reports.r2.items;if(d.length===0)return'<div class="card"><h2>Tüm Bulgular (R2)</h2><p class="empty">Bulgu yok.</p></div>';const u=`
    <div class="filter-bar">
      <label>Önem filtrele:
        <select id="sev-filter">
          <option value="">Tümü</option>
          <option value="CRITICAL">KRİTİK</option>
          <option value="HIGH">YÜKSEK</option>
          <option value="MEDIUM">ORTA</option>
          <option value="LOW">DÜŞÜK</option>
          <option value="INFO">BİLGİ</option>
        </select>
      </label>
      <label>Sınıf filtrele:
        <select id="cls-filter">
          <option value="">Tümü</option>
          <option value="SPEC">GTFS Geçerliliği</option>
          <option value="INTEROP">GTFS Uyumluluğu</option>
          <option value="QUALITY">GTFS Kalitesi</option>
          <option value="ANALYTICS">GTFS Analitiği</option>
        </select>
      </label>
      <label>Kural filtrele:
        <select id="rule-filter">
          <option value="">Tümü</option>
          ${[...new Map(d.filter(l=>t.has(l.notice_id)).map(l=>{const r=t.get(l.notice_id);return[r.rule_id,r.title||r.rule_id]})).entries()].sort(([l],[r])=>l.localeCompare(r)).map(([l,r])=>`<option value="${w(l)}">${w(l)} — ${w(r)}</option>`).join("")}
        </select>
      </label>
      <span id="filter-count" class="filter-count"></span>
    </div>`,a=d.map(l=>{const r=t.get(l.notice_id);if(!r)return"";const n=s.get(r.rule_id),i=n!=null&&n.count>0?n.pubDelta/n.count:null,c=n!=null&&n.count>0?n.qualDelta/n.count:null,f=i!=null&&i>=.005?`<span class="pub-delta">+${i.toFixed(2)}</span>`:'<span class="muted-text">—</span>',g=c!=null&&c>=.005?`<span class="score-delta">+${c.toFixed(2)}</span>`:'<span class="muted-text">—</span>',b=We(r,o)?`<button class="map-pin-btn" data-notice-id="${w(r.id)}" title="Haritada göster" aria-label="Haritada göster">📍</button>`:"";return`
      <tr data-severity="${r.severity}" data-class="${r.rule_class}" data-rule="${r.rule_id}">
        <td>${w(l.display_label)}</td>
        <td style="color:${j[r.severity]}">${O[r.severity]}</td>
        <td>${U[r.rule_class]}</td>
        <td>${w(r.message)}</td>
        <td>${r.file?w(r.file):"—"}</td>
        <td>${r.line??"—"}</td>
        <td>${r.field?w(r.field):"—"}</td>
        <td class="score-delta-cell">${f}</td>
        <td class="score-delta-cell">${g}</td>
        <td class="map-btn-cell">${b}</td>
      </tr>`}).join("");return`
    <div class="card" id="r2-card">
      <h2>Tüm Bulgular (R2) <span class="count-badge">${d.length}</span></h2>
      ${u}
      <div id="r2-cap-warning" class="cap-warning" hidden></div>
      <div class="table-scroll">
        <table class="data-table" id="r2-table">
          <thead><tr>
            <th>Kural</th><th>Önem</th><th>Sınıf</th><th>Mesaj</th>
            <th>Dosya</th><th>Satır</th><th>Alan</th>
            <th class="score-delta-cell">+Yayın <span class="col-info" title="Bu hatayı düzeltmenin yayın skoruna katkısı (turuncu). Toplamı = 100 − mevcut yayın skoru.">ℹ</span></th>
            <th class="score-delta-cell">+Kalite <span class="col-info" title="Bu hatayı düzeltmenin kalite skoruna katkısı (yeşil). Toplamı = 100 − mevcut kalite skoru.">ℹ</span></th>
            <th class="map-btn-cell"></th>
          </tr></thead>
          <tbody>${a}</tbody>
        </table>
      </div>
    </div>`}function w(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function y(e,t,s,o){const d=t.stop_coords[e];if(!d)return null;const p=`<strong>${t.stops[e]??e}</strong>${o?` ${o}`:""}<br><code>${e}</code>`;return{lat:d[0],lon:d[1],label:p,primary:s}}const Ve=new Set(["STP_003","STP_004","STP_005","STP_006","STP_007","STP_008","STP_009","STP_010","STP_011","STP_012","STP_013","STP_014","STP_015","STP_016","STP_017","STP_018","STP_019","STP_020","STP_021","STP_022","STP_023","STP_024","STP_025","STP_026","STP_027","STP_028","STP_029","STP_030","GEO_001","GEO_002","GEO_003","GEO_004","GEO_005","GEO_009","GEO_010","GEO_011","GEO_012","GEO_014","PTH_012","PTH_013","PTH_015","PTH_016","PTH_017","PTH_018","PTH_019","LVL_006","SHP_022","VAT_007"]),Q=new Set(["TRP_001","TRP_002","TRP_003","TRP_004","TRP_005","TRP_006","TRP_007","TRP_008","TRP_009","TRP_010","TRP_011","TRP_012","TRP_013","TRP_014","TRP_015","TRP_016","TRP_017","TRP_018","TRP_019","TRP_020","FRQ_001","FRQ_002","FRQ_003","FRQ_004","FRQ_005","FRQ_006","FRQ_007","FRQ_008","FRQ_009","STM_001","STM_002","STM_003","STM_004","STM_005","STM_006","STM_007","STM_008","STM_009","STM_010","STM_011","STM_013","STM_015","STM_016","STM_018","STM_019","STM_021","STM_022","STM_023","STM_024","STM_027","STM_028","STM_029","STM_030","STM_031","STM_032","STM_034","OPR_006","OPR_017"]),J=new Set(["RTS_001","RTS_002","RTS_003","RTS_004","RTS_005","RTS_006","RTS_007","RTS_008","RTS_009","RTS_010","RTS_011","RTS_012","RTS_013","RTS_014","RTS_015","RTS_016","RTS_017","OPR_001","OPR_002","OPR_003"]);function We(e,t){const s=e.entity_id??"";if(Ve.has(e.rule_id))return!!(s&&s in t.stop_coords);if(e.rule_id==="GEO_013")return Object.keys(t.stop_coords).length>0;if(e.rule_id==="SHP_017"){const o=e.message.match(/\(kod: '([^']+)'\)/)?.[1],d=e.details?.ctx_b??"",h=e.details?.ctx_a??"";return!!(o&&o in t.stop_coords)||d.length>0||h.length>0}if(e.rule_id==="STM_014"){const o=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/);return o&&(o[1]in t.stop_coords||o[2]in t.stop_coords)?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_020"){const o=e.details?.stop_a??"",d=e.details?.stop_b??"";return o&&o in t.stop_coords||d&&d in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_025"){const o=e.details?.stop_a??"",d=e.details?.stop_b??"";return o&&o in t.stop_coords||d&&d in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="OPR_015"){const o=e.details?.shape_id??"";return!!(o&&o in t.shape_coords)}if(e.rule_id==="PTH_014"){const o=e.details?.from_station??"",d=e.details?.to_station??"";return!!(o&&o in t.stop_coords&&d&&d in t.stop_coords)}if(["OPR_007","OPR_008","STM_017"].includes(e.rule_id))return!!(s&&s in t.trip_shapes);if(["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"].includes(e.rule_id))return!!(s&&s in t.shape_coords);if(["STM_012","STM_026","STM_033"].includes(e.rule_id))return!!(s&&s in t.trip_shapes)||!!(s&&s in t.trip_stops);if(e.rule_id==="STM_035"){const o=e.observed_value??"";return!!(o&&o in t.stop_coords)||!!(s&&(s in t.trip_shapes||s in t.trip_stops))}return e.rule_id==="VAT_005"?Object.keys(t.stop_coords).length>0:Q.has(e.rule_id)?!!(s&&(s in t.trip_shapes||s in t.trip_stops)):J.has(e.rule_id)?!!(s&&(t.route_shapes[s]?.length??0)>0):!1}function Ze(e,t){const s=e.entity_id??"";if(e.rule_id==="SHP_017"){const a=e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",l=e.details?.shape_id??"",r=(e.details?.ctx_b??"").split(",").filter(Boolean),n=(e.details?.ctx_a??"").split(",").filter(Boolean),i=(e.details?.seq_b??"").split(",").filter(Boolean),c=(e.details?.seq_a??"").split(",").filter(Boolean),f=e.observed_value??"",g=[];for(let k=0;k<r.length;k++){const S=i[k]?`#${i[k]} `:"",R=y(r[k],t,!0,`${S}(önceki)`);R&&g.push(R)}const b=f?`#${f} `:"",m=a?y(a,t,!1,`${b}⚠ sıra bozuk`):null;m&&g.push(m);for(let k=0;k<n.length;k++){const S=c[k]?`#${c[k]} `:"",R=y(n[k],t,!0,`${S}(sonraki)`);R&&g.push(R)}const _=l?t.shape_coords[l]??[]:[],v=[];return _.length>1&&v.push({color:"#f59e0b",label:"Güzergah şekli"}),v.push({color:"#2563eb",label:"Komşu duraklar"}),v.push({color:"#dc2626",label:"Sırası bozuk durak"}),{pins:g,polyline:_.length>1?_:void 0,legendItems:v,showArrows:_.length>1}}if(e.rule_id==="STM_014"){const a=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/),l=a?y(a[1].trim(),t,!0,"(kalkış)"):null,r=a?y(a[2].trim(),t,!1,"(varış)"):null,n=[l,r].filter(g=>g!==null),i=s?t.trip_shapes[s]??"":"",c=i?t.shape_coords[i]??[]:[],f=[];return c.length>1&&f.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>1&&(f.push({color:"#2563eb",label:"Kalkış durağı"}),f.push({color:"#dc2626",label:"Varış durağı"})),{pins:n,polyline:c.length>1?c:void 0,legendItems:f,showArrows:c.length>1}}if(e.rule_id==="STM_020"){const a=e.details?.stop_a?y(e.details.stop_a,t,!0,"(kalkış)"):null,l=e.details?.stop_b?y(e.details.stop_b,t,!1,"(varış)"):null,r=[a,l].filter(f=>f!==null),n=s?t.trip_shapes[s]??"":"",i=n?t.shape_coords[n]??[]:[],c=[];return i.length>1&&c.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&c.push({color:"#2563eb",label:"Kalkış durağı"}),r.length>1&&c.push({color:"#dc2626",label:"Varış durağı"}),{pins:r,polyline:i.length>1?i:void 0,legendItems:c,showArrows:!0}}if(e.rule_id==="SHP_010"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/^\(([^,]+),([^)]+)\)$/),r=[];if(l){const i=parseFloat(l[1]),c=parseFloat(l[2]);!isNaN(i)&&!isNaN(c)&&r.push({lat:i,lon:c,label:`Tekrarlanan koordinat<br><code>(${l[1]}, ${l[2]})</code>`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&n.push({color:"#dc2626",label:"Tekrarlanan nokta"}),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_022"){const a=e.message.match(/durağı '([^']+)' güzergah şeklinin/)?.[1]??"",l=y(s,t,!1,"⚠ belirsiz eşleşme"),r=a?t.shape_coords[a]??[]:[],n=l?[l]:[],i=[];return r.length>1&&i.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&i.push({color:"#dc2626",label:"Belirsiz durak"}),{pins:n,polyline:r.length>1?r:void 0,legendItems:i,showArrows:!0}}if(e.rule_id==="SHP_018"||e.rule_id==="SHP_019"){const a=s?t.shape_coords[s]??[]:[],l=a.length>1?[{color:"#f59e0b",label:"Kullanılmayan güzergah şekli"}]:[];return{pins:[],polyline:a.length>1?a:void 0,legendItems:l,showArrows:!0}}if(e.rule_id==="OPR_007"){const a=e.details?.dup_stop??e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",l=(e.details?.stops??"").split(",").filter(Boolean),r=s?t.trip_shapes[s]??"":"",n=r?t.shape_coords[r]??[]:[],i=new Set,c=[];for(const g of l){if(i.has(g))continue;i.add(g);const b=g===a,m=y(g,t,!b,b?"(tekrar eden)":void 0);m&&(m.small=!b,c.push(m))}const f=[];return n.length>1&&f.push({color:"#f59e0b",label:"Güzergah şekli"}),c.length>1&&f.push({color:"#2563eb",label:"Duraklar"}),a&&f.push({color:"#dc2626",label:"Tekrar eden durak"}),{pins:c,polyline:n.length>1?n:void 0,legendItems:f,showArrows:!0}}if(e.rule_id==="STM_017"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(c=>y(c,t,!0)).filter(c=>c!==null).map(c=>({...c,small:!0})),i=[];return l.length>1&&i.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&i.push({color:"#2563eb",label:"Sefer durakları"}),{pins:n,polyline:l.length>1?l:void 0,legendItems:i,showArrows:!0}}if(e.rule_id==="GEO_006"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/segment\s+(\d+)→(\d+)/),r=[];if(l&&a.length>0){const i=parseInt(l[1],10),c=parseInt(l[2],10),f=a[i],g=a[c];f&&r.push({lat:f[0],lon:f[1],label:`<strong>Atlama başlangıcı</strong><br>Segment ${i}`,primary:!0}),g&&r.push({lat:g[0],lon:g[1],label:`<strong>Atlama bitişi</strong><br>Segment ${c}`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&(n.push({color:"#dc2626",label:"Atlama başlangıcı"}),n.push({color:"#2563eb",label:"Atlama bitişi"})),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!1}}if(e.rule_id==="SHP_020"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/\(([^,]+),([^)]+)\)$/),r=[];if(l){const i=parseFloat(l[1]),c=parseFloat(l[2]);!isNaN(i)&&!isNaN(c)&&r.push({lat:i,lon:c,label:`Tekrarlayan koordinat<br><code>(${l[1]}, ${l[2]})</code>`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&n.push({color:"#dc2626",label:"Tekrarlayan nokta"}),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_009"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/segment (\d+)-(\d+) ∩ (\d+)-(\d+)/),r=[];if(l&&a.length>0){const i=parseInt(l[1],10),c=parseInt(l[2],10),f=parseInt(l[3],10),g=parseInt(l[4],10),b=a[i],m=a[c],_=a[f],v=a[g];b&&r.push({lat:b[0],lon:b[1],label:`Segment ${i}→${c} başlangıcı`,primary:!0}),m&&r.push({lat:m[0],lon:m[1],label:`Segment ${i}→${c} bitişi`,primary:!0}),_&&r.push({lat:_[0],lon:_[1],label:`Segment ${f}→${g} başlangıcı`,primary:!1}),v&&r.push({lat:v[0],lon:v[1],label:`Segment ${f}→${g} bitişi`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&(n.push({color:"#2563eb",label:"1. kesişen segment"}),n.push({color:"#dc2626",label:"2. kesişen segment"})),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="OPR_008"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=[];l.length>1&&r.push({color:"#9ca3af",label:"Güzergah şekli"});const n=[],i=[],c=new Set;for(let f=0;;f++){const g=e.details?.[`bad_seg_${f}_a`],b=e.details?.[`bad_seg_${f}_b`];if(!g||!b)break;const m=t.stop_coords[g],_=t.stop_coords[b];m&&_&&n.push({coords:[[m[0],m[1]],[_[0],_[1]]],color:"#dc2626",weight:5,zoomTo:f===0}),m&&!c.has(g)&&(c.add(g),i.push({lat:m[0],lon:m[1],label:`<code>${g}</code>`,primary:!0,small:!1})),_&&!c.has(b)&&(c.add(b),i.push({lat:_[0],lon:_[1],label:`<code>${b}</code>`,primary:!0,small:!1}))}return n.length>0&&r.push({color:"#dc2626",label:"Bozuk segment"}),r.push({color:"#2563eb",label:"Segment uçları"}),{pins:i,polyline:l.length>1?l.map(f=>[f[0],f[1]]):void 0,extraPolylines:n,legendItems:r,showArrows:!1}}if(e.rule_id==="STM_025"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",i=e.details?.stop_b??"",c=[];for(const m of r){if(m===n||m===i)continue;const _=y(m,t,!0);_&&c.push({..._,small:!0})}const f=n?y(n,t,!0,"(kalkış)"):null;f&&c.push(f);const g=i?y(i,t,!1,"(varış — çok kısa süre)"):null;g&&c.push(g);const b=[];return l.length>1&&b.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&b.push({color:"#2563eb",label:"Sefer durakları"}),b.push({color:"#dc2626",label:"Kısa süreli segment sonu"}),{pins:c,polyline:l.length>1?l:void 0,legendItems:b,showArrows:!0}}if(e.rule_id==="OPR_015"){const a=e.details?.shape_id??"",l=a?t.shape_coords[a]??[]:[],r=l.length>1?[{color:"#f59e0b",label:"Tek güzergah şekli"}]:[];return{pins:[],polyline:l.length>1?l:void 0,legendItems:r,showArrows:!0}}function o(a){const l=t.shape_trips[a]??"";return(l?t.trip_stops[l]??[]:[]).map(n=>y(n,t,!0)).filter(n=>n!==null).map(n=>({...n,small:!0}))}if(e.rule_id==="SHP_007"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:a.length>1}}if(e.rule_id==="SHP_012"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:!0}}if(e.rule_id==="SHP_014"){const a=s?t.shape_coords[s]??[]:[],l=e.details?.problem_stop??"",r=e.details?.trip_id??"",n=r?t.trip_stops[r]??[]:[],i=[];for(const b of n){if(b===l)continue;const m=y(b,t,!0);m&&i.push({...m,small:!0})}const c=e.details?.endpoint==="start"?"⚠ ilk durak — shape başından uzak":"⚠ son durak — shape bitişinden uzak",f=l?y(l,t,!1,c):null;f&&i.push(f);const g=[];return a.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),f&&g.push({color:"#dc2626",label:"Uçtan uzak durak"}),{pins:i,polyline:a.length>1?a:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="SHP_015"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:!0}}if(e.rule_id==="SHP_016"){const a=s?t.shape_coords[s]??[]:[],l=e.details?.first_stop??"",r=e.details?.trip_id??"",n=r?t.trip_stops[r]??[]:[],i=[];for(const g of n){if(g===l)continue;const b=y(g,t,!0);b&&i.push({...b,small:!0})}const c=l?y(l,t,!1,"⚠ ilk durak — shape'in yanlış ucuna düşüyor"):null;c&&i.push(c);const f=[];return a.length>1&&f.push({color:"#f59e0b",label:"Güzergah şekli (ters)"}),n.length>0&&f.push({color:"#2563eb",label:"Sefer durakları"}),c&&f.push({color:"#dc2626",label:"İlk durak (yanlış uçta)"}),{pins:i,polyline:a.length>1?a:void 0,legendItems:f,showArrows:!0}}if(e.rule_id==="STM_012"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",i=e.details?.stop_b??"",c=[];for(const m of r){if(m===n||m===i)continue;const _=y(m,t,!0);_&&c.push({..._,small:!0})}const f=n?y(n,t,!0,"(kalkış — imkansız hız)"):null,g=i?y(i,t,!1,"(varış — imkansız hız)"):null;f&&c.push(f),g&&c.push(g);const b=[];return l.length>1&&b.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&b.push({color:"#2563eb",label:"Sefer durakları"}),b.push({color:"#dc2626",label:"İmkansız hız segmenti"}),{pins:c,polyline:l.length>1?l:void 0,legendItems:b,showArrows:!0}}if(e.rule_id==="STM_026"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",i=e.details?.stop_b??"",c=[];for(const m of r){if(m===n||m===i)continue;const _=y(m,t,!0);_&&c.push({..._,small:!0})}const f=n?y(n,t,!0,"(kalkış)"):null,g=i?y(i,t,!1,"(varış — çok uzak)"):null;f&&c.push(f),g&&c.push(g);const b=[];return l.length>1&&b.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&b.push({color:"#2563eb",label:"Sefer durakları"}),b.push({color:"#dc2626",label:"Aşırı uzak segment sonu"}),{pins:c,polyline:l.length>1?l:void 0,legendItems:b,showArrows:!0}}if(e.rule_id==="STM_033"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(c=>y(c,t,!1,"(tek durak — sefer kullanılamaz)")).filter(c=>c!==null),i=[];return l.length>1&&i.push({color:"#f59e0b",label:"Güzergah şekli"}),i.push({color:"#dc2626",label:"Tek durak (kullanılamaz sefer)"}),{pins:n,polyline:l.length>1?l:void 0,legendItems:i,showArrows:l.length>1}}if(e.rule_id==="STM_035"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.observed_value??"",i=[];for(const g of r){if(g===n)continue;const b=y(g,t,!0);b&&i.push({...b,small:!0})}const c=n?y(n,t,!1,"(ardışık iki kez ziyaret)"):null;c&&i.push(c);const f=[];return l.length>1&&f.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&f.push({color:"#2563eb",label:"Sefer durakları"}),c&&f.push({color:"#dc2626",label:"Tekrar ziyaret edilen durak"}),{pins:i,polyline:l.length>1?l:void 0,legendItems:f,showArrows:l.length>1}}if(e.rule_id==="PTH_014"){const a=e.details?.from_station??"",l=e.details?.to_station??"",r=t.stop_coords[a],n=t.stop_coords[l];if(!r||!n)return defaultMapOptions(e,t);const i=t.stops[a]??a,c=t.stops[l]??l;return{center:[(r[0]+n[0])/2,(r[1]+n[1])/2],zoom:15,markers:[{lat:r[0],lon:r[1],color:"#ef4444",label:i,title:`İstasyon: ${i}`},{lat:n[0],lon:n[1],color:"#3b82f6",label:c,title:`İstasyon: ${c}`}],polylines:[{coords:[[r[0],r[1]],[n[0],n[1]]],color:"#ef4444",dashArray:"6,4",weight:2}],showArrows:!1}}if(e.rule_id==="VAT_005"){const a=new Set((e.details?.isolated_stops??"").split(",").filter(Boolean)),l=new Set;for(const n of Object.values(t.trip_stops))for(const i of n)l.add(i);const r=[];for(const[n,i]of Object.entries(t.stop_coords)){const c=a.has(n),f=l.has(n);if(!c&&!f)continue;const g=t.stops[n]??n;r.push({lat:i[0],lon:i[1],label:c?`<strong>⚠ İzole</strong><br><strong>${g}</strong><br><code>${n}</code>`:`<strong>${g}</strong><br><code>${n}</code>`,primary:!c,small:!c})}return{pins:r,legendItems:[{color:"#2563eb",label:"Bağlı duraklar"},{color:"#dc2626",label:"İzole duraklar"}]}}if(e.rule_id==="VAT_007"){const a=(e.details?.routes??"").split(",").filter(Boolean),l=["#3b82f6","#10b981","#8b5cf6","#f97316","#06b6d4"],r=t.stop_coords[s],n=t.stops[s]??s,i=r?[{lat:r[0],lon:r[1],label:`<strong>Terminal: ${n}</strong><br><code>${s}</code>`,primary:!1}]:[],c=[],f=[];i.length>0&&f.push({color:"#dc2626",label:"Terminal durağı"});let g=0;for(const b of a.slice(0,5)){const m=t.route_shapes[b]??[],_=l[g%l.length];let v=!1;for(const k of m.slice(0,2)){const S=t.shape_coords[k]??[];S.length>1&&(c.push({coords:S,color:_,weight:3,zoomTo:!1}),v=!0)}if(v){const k=t.routes[b]??b;f.push({color:_,label:k}),g++}}return{pins:i,extraPolylines:c,legendItems:f,showArrows:!1}}if(e.rule_id==="GEO_013"){const a=Object.entries(t.stop_coords).map(([l,r])=>({lat:r[0],lon:r[1],label:t.stops[l]?`<strong>${t.stops[l]}</strong><br><code>${l}</code>`:`<code>${l}</code>`,primary:!0,small:!0}));return{pins:a,legendItems:[{color:"#2563eb",label:`${a.length} durak`}]}}if(Q.has(e.rule_id)){const a=t.trip_shapes[s]??"",l=a?t.shape_coords[a]??[]:[],r=t.trip_stops[s]??[],n=t.trip_routes[s]??"",i=n?t.routes[n]??"":"",c=r.flatMap((g,b)=>{const m=y(g,t,!0,`#${b+1}`);return m?[{...m,small:!0}]:[]}),f=[];return l.length>1&&f.push({color:"#f59e0b",label:i?`Güzergah şekli — ${i}`:"Güzergah şekli"}),c.length>0&&f.push({color:"#2563eb",label:"Sefer durakları"}),{pins:c,polyline:l.length>1?l:void 0,legendItems:f,showArrows:l.length>1}}if(J.has(e.rule_id)){const a=t.route_shapes[s]??[],l=t.routes[s]??s,r=[];for(const n of a.slice(0,4)){const i=t.shape_coords[n]??[];i.length>1&&r.push({coords:i,color:"#f59e0b",weight:3,zoomTo:r.length===0})}return r.length===0?{pins:[]}:{pins:[],extraPolylines:r,legendItems:[{color:"#f59e0b",label:`Güzergah şekilleri — ${l}`}],showArrows:!1}}const d=t.stop_coords[s];if(!d)return{pins:[]};const[h,p]=d,u=t.stops[s]??s;if(e.rule_id==="STP_029"){const a=e.message.match(/üst istasyonundan \(([^)]+)\)/)?.[1]??"",l=[{lat:h,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],r=a?y(a,t,!1,"(üst istasyon)"):null;r&&l.push(r);const n=l.length>1?[{color:"#2563eb",label:"Durak"},{color:"#dc2626",label:"Üst İstasyon"}]:[];return{pins:l,legendItems:n}}if(e.rule_id==="STP_016"){const a=e.observed_value?.match(/== '([^']+)'/)?.[1]??"",l=a?t.stops[a]??a:"",r=l?`<strong>${u}</strong> = <strong>${l}</strong><br><code>${s}</code> ve <code>${a}</code>`:`<strong>${u}</strong><br><code>${s}</code>`;return{pins:[{lat:h,lon:p,label:r,primary:!0}]}}if(e.rule_id==="STP_017"){const a=e.observed_value?.match(/to '([^']+)'/)?.[1]??"",l=[{lat:h,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],r=a?y(a,t,!1):null;r&&l.push(r);const n=l.length>1?[{color:"#2563eb",label:"Durak 1"},{color:"#dc2626",label:"Durak 2"}]:[];return{pins:l,legendItems:n}}if(e.rule_id==="GEO_009"){const a=e.observed_value?.match(/\(shape '([^']+)'\)/)?.[1]??"",l=a?t.shape_coords[a]??[]:[],r=[{color:"#2563eb",label:"Durak"}];return l.length>0&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),{pins:[{lat:h,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],polyline:l.length>1?l:void 0,legendItems:r}}return{pins:[{lat:h,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}]}}function Qe(e,t,s){if(e.querySelectorAll(".r9-main-row").forEach(i=>{i.addEventListener("click",()=>{const c=i.dataset.idx,f=e.querySelector(`.r9-detail-row[data-for="${c}"]`);if(!f)return;f.hidden=!f.hidden;const g=i.querySelector(".r9-arrow");g&&(g.textContent=f.hidden?"▶":"▼")})}),t){const i=new Map(t.notices.map(c=>[c.id,c]));e.querySelectorAll(".map-pin-btn").forEach(c=>{c.addEventListener("click",f=>{f.stopPropagation();const g=c.dataset.noticeId??"",b=i.get(g);if(!b)return;const m=Ze(b,t.name_index);if(m.pins.length===0&&!m.polyline)return;const _=new Set(["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"]),v=b.entity_id??"";let k;if(_.has(b.rule_id))k=`şekil: ${v}`;else if(Q.has(b.rule_id)){const S=t.name_index.trip_routes[v]??"",R=S?t.name_index.routes[S]??"":"",M=t.name_index.trips[v]??"";k=R?`${R}${M?` — ${M}`:""}`:M||v}else J.has(b.rule_id)?k=t.name_index.routes[v]??v:k=t.name_index.stops[v]??v;Be(`${b.rule_id} — ${k}`,m)})})}const o=e.querySelector("#sev-filter"),d=e.querySelector("#cls-filter"),h=e.querySelector("#rule-filter"),p=e.querySelector("#r2-table");if(!o||!d||!p)return;const u=h?Array.from(h.options).filter(i=>i.value!=="").map(i=>[i.value,i.text]):[],a=[["CRITICAL","KRİTİK"],["HIGH","YÜKSEK"],["MEDIUM","ORTA"],["LOW","DÜŞÜK"],["INFO","BİLGİ"]],l=[["SPEC","GTFS Geçerliliği"],["INTEROP","GTFS Uyumluluğu"],["QUALITY","GTFS Kalitesi"],["ANALYTICS","GTFS Analitiği"]];function r(i,c,f,g){i.innerHTML='<option value="">Tümü</option>'+g.filter(([b])=>f.has(b)).map(([b,m])=>`<option value="${b}"${b===c?" selected":""}>${m}</option>`).join(""),c&&!f.has(c)&&(i.value="")}function n(){const i=o.value,c=d.value,f=h?.value??"";let g=0,b=0;const m=new Set,_=new Set,v=new Set;p.querySelectorAll("tbody tr").forEach(z=>{b++;const B=z.dataset.severity??"",ee=z.dataset.class??"",te=z.dataset.rule??"",x=!i||B===i,N=!c||ee===c,I=!f||te===f;z.style.display=x&&N&&I?"":"none",x&&N&&I&&g++,N&&I&&m.add(B),x&&I&&_.add(ee),x&&N&&v.add(te)});const k=i||c||f,S=e.querySelector("#filter-count");S&&(S.textContent=k?`${b} noticeden ${g} tanesi`:"");const R=e.querySelector("#r2-card h2 .count-badge");R&&(R.textContent=String(k?g:b));const M=e.querySelector("#r2-cap-warning");if(M){const z=f&&s?s[f]:void 0;z!=null?(M.hidden=!1,M.textContent=`⚠ Bu kural cap'e çarptı — raporda yalnızca ${g.toLocaleString("tr-TR")} notice gösterilmektedir, gerçek toplam: ${z.toLocaleString("tr-TR")}.`):M.hidden=!0}r(o,i,m,a),r(d,c,_,l),h&&(h.innerHTML='<option value="">Tümü</option>'+u.filter(([z])=>v.has(z)).map(([z,B])=>`<option value="${z}"${z===f?" selected":""}>${w(B)}</option>`).join(""),f&&!v.has(f)&&(h.value=""))}o.addEventListener("change",n),d.addEventListener("change",n),h?.addEventListener("change",n)}const F={CRITICAL:0,HIGH:1,MEDIUM:2,LOW:3,INFO:4},Je={Stop:"Durak",Trip:"Sefer",Route:"Hat",Shape:"Güzergah",Agency:"İşletici",Service:"Servis",File:"Dosya",Feed:"Feed",Row:"Satır",Field:"Alan",Transfer:"Aktarma",Pathway:"Geçit",Level:"Kat",Translation:"Çeviri",Attribution:"Atıf",Fare:"Ücret"};function Xe(e,t){const s=et(t.notices),o=t.notices.length;e.innerHTML=`
    <div class="rules-page">
      <div class="rules-summary-bar">
        <span class="rules-summary-text">${o} bulgu · ${s.length} kural</span>
      </div>
      <div class="rules-accordion">
        ${s.map(d=>tt(d)).join("")}
      </div>
    </div>`,st(e)}function et(e){const t=new Map;for(const s of e){t.has(s.rule_id)||t.set(s.rule_id,{rule_id:s.rule_id,max_severity:s.severity,rule_class:s.rule_class,notices:[]});const o=t.get(s.rule_id);o.notices.push(s),(F[s.severity]??9)<(F[o.max_severity]??9)&&(o.max_severity=s.severity)}return Array.from(t.values()).sort((s,o)=>{const d=(F[s.max_severity]??9)-(F[o.max_severity]??9);return d!==0?d:o.notices.length-s.notices.length})}function tt(e){const t=j[e.max_severity],s=O[e.max_severity],o=U[e.rule_class]??e.rule_class,d=e.rule_class.toLowerCase(),p=[...e.notices].sort((a,l)=>(F[a.severity]??9)-(F[l.severity]??9)).map(a=>{const l=lt(a.message),r=l.length>160?l.slice(0,160)+"…":l,n=j[a.severity]??"#666",i=Je[a.entity_type]??a.entity_type,c=a.entity_id?`<span class="muted-text">${A(i)}</span> <code class="entity-id">${A(a.entity_id)}</code>`:`<span class="muted-text">${A(i)}</span>`;return`
      <tr>
        <td style="color:${n};white-space:nowrap">${O[a.severity]}</td>
        <td>${c}</td>
        <td class="msg-cell">${A(r)}</td>
      </tr>`}).join(""),u=e.notices[0]?.remediation??"";return`
    <div class="rule-acc-section">
      <button class="rule-acc-header" type="button" aria-expanded="false">
        <span class="acc-arrow">▶</span>
        <span class="rule-title-block">
          <code class="rule-code rule-code-lg">${A(e.rule_id)}</code>
          ${u?`<span class="rule-desc-text">${A(u)}</span>`:""}
        </span>
        <span class="badge badge-${d}">${A(o)}</span>
        <span class="rule-sev-badge" style="color:${t}">${A(s)}</span>
        <span class="acc-count rule-notice-count">${e.notices.length}</span>
      </button>
      <div class="rule-acc-body" hidden>
        <div class="table-scroll">
          <table class="data-table">
            <thead><tr>
              <th>Önem</th><th>Varlık</th><th>Mesaj</th>
            </tr></thead>
            <tbody>${p}</tbody>
          </table>
        </div>
      </div>
    </div>`}function st(e){e.querySelectorAll(".rule-acc-header").forEach(t=>{t.addEventListener("click",()=>{const o=t.closest(".rule-acc-section").querySelector(".rule-acc-body"),d=t.querySelector(".acc-arrow"),h=!o.hidden;o.hidden=h,d.textContent=h?"▶":"▼",t.setAttribute("aria-expanded",String(!h))})})}function lt(e){return e.replace(/\btrip_id\b/g,"sefer").replace(/\bstop_sequence\b/g,"Durak Sırası").replace(/\barrival_time\b/g,"Varış zamanı").replace(/\bdeparture_time\b/g,"Kalkış zamanı").replace(/\bheadway\b/g,"Sefer Aralığı")}function A(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function at(e,t,s){e.innerHTML=`
    <section class="page-export-wide">
      <div class="card">
        <h2>Dışa Aktar</h2>
        <p class="export-filename">Kaynak: <code>${E(s)}</code></p>
        <div class="export-actions">
          <button id="btn-export-html" class="btn btn-primary">HTML Rapor İndir</button>
          <button id="btn-export-csv"  class="btn btn-secondary">CSV İndir</button>
          <button id="btn-export-json" class="btn btn-secondary">JSON Ham Veri İndir</button>
          <button id="btn-export-pdf"  class="btn btn-secondary">PDF Olarak Yazdır</button>
        </div>
      </div>
    </section>`,e.querySelector("#btn-export-html").addEventListener("click",()=>nt(t,s)),e.querySelector("#btn-export-csv").addEventListener("click",()=>ot(t,s)),e.querySelector("#btn-export-json").addEventListener("click",()=>rt(t,s)),e.querySelector("#btn-export-pdf").addEventListener("click",()=>it(t,s))}function re(e){const t=e==null?"":String(e),s=/^[=+\-@]/.test(t)?`'${t}`:t;return s.includes(",")||s.includes('"')||s.includes(`
`)?`"${s.replace(/"/g,'""')}"`:s}function ot(e,t){const s=["Kural","Önem","Sınıf","Mesaj","Varlık ID","Dosya","Satır"],o=e.notices.map(u=>[u.rule_id,O[u.severity]??u.severity,U[u.rule_class]??u.rule_class,u.message,u.entity_id??"",u.file??"",u.line!=null?String(u.line):""].map(re).join(",")),h="\uFEFF"+[s.map(re).join(","),...o].join(`\r
`),p=new Blob([h],{type:"text/csv; charset=utf-8"});X(p,t.replace(/\.zip$/i,"-report.csv"))}function rt(e,t){const s=new Blob([JSON.stringify(e,null,2)],{type:"application/json"});X(s,t.replace(/\.zip$/i,"-report.json"))}function me(e,t){const{r1:s,r5:o}=e.reports,d=s.publishable?s.conditional?"Koşullu Yayına Uygun":"Yayına Uygun":"Yayınlanması Tavsiye Edilmez",h=e.notices.map(p=>`
    <tr>
      <td>${E(p.rule_id)}</td>
      <td>${O[p.severity]}</td>
      <td>${U[p.rule_class]}</td>
      <td>${E(p.message)}</td>
      <td>${p.entity_id?E(p.entity_id):""}</td>
      <td>${p.file?E(p.file):""}</td>
      <td>${p.line??""}</td>
    </tr>`).join("");return`<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8"/>
  <title>GTFS Doğrulama Raporu - ${E(t)}</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 1100px; margin: 0 auto; padding: 2rem; color: #1e293b; }
    h1 { font-size: 1.4rem; margin-bottom: .5rem; }
    .summary { display: flex; gap: 2rem; flex-wrap: wrap; margin: 1rem 0 1.5rem; }
    .kpi { display: flex; flex-direction: column; }
    .kpi-value { font-size: 1.8rem; font-weight: 700; }
    .kpi-label { font-size: .8rem; color: #64748b; }
    .score-breakdown { margin-top: .3rem; font-size: .8rem; color: #64748b; }
    h2 { font-size: 1rem; margin: 1.5rem 0 .5rem; }
    table { border-collapse: collapse; width: 100%; font-size: 0.82rem; }
    th, td { border: 1px solid #e2e8f0; padding: 0.35rem 0.6rem; text-align: left; vertical-align: top; }
    th { background: #f8fafc; font-weight: 600; }
    @media print { body { padding: 1rem; } }
  </style>
</head>
<body>
  <h1>GTFS Doğrulama Raporu</h1>
  <p style="color:#64748b;font-size:.9rem">Kaynak: ${E(t)}</p>

  <div class="summary">
    <div class="kpi">
      <span class="kpi-value">${d}</span>
      <span class="kpi-label">Yayınlanabilirlik</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${o.score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">Kalite Skoru</span>
      <span class="score-breakdown">GTFS Geçerliliği ${o.spec_score.toFixed(1)} · GTFS Uyumluluğu ${o.interop_score.toFixed(1)} · GTFS Kalitesi ${o.quality_score.toFixed(1)} · GTFS Analitiği ${o.analytics_score.toFixed(1)}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${o.pub_score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">Yayın Skoru</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${e.notices.length}</span>
      <span class="kpi-label">Toplam Bulgu</span>
    </div>
  </div>

  <h2>Bulgular</h2>
  <table>
    <thead><tr>
      <th>Kural</th><th>Önem</th><th>Sınıf</th><th>Mesaj</th>
      <th>Varlık ID</th><th>Dosya</th><th>Satır</th>
    </tr></thead>
    <tbody>${h}</tbody>
  </table>
</body>
</html>`}function nt(e,t){const s=me(e,t),o=new Blob([s],{type:"text/html; charset=utf-8"});X(o,t.replace(/\.zip$/i,"-report.html"))}function it(e,t){const s=me(e,t),o=window.open("","_blank");if(!o){alert("Açılır pencere engellendi. Tarayıcı ayarlarını kontrol edin.");return}o.document.write(s),o.document.close(),o.focus(),o.print()}function X(e,t){const s=URL.createObjectURL(e),o=document.createElement("a");o.href=s,o.download=t,o.click(),URL.revokeObjectURL(s)}function E(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}const ne={domain:"Rapor",fix:"Ayrıntı ve Düzeltme",rules:"Kategori Bazlı",export:"Dışa Aktar"};function ct(){localStorage.getItem("gtfs-theme")==="dark"&&document.documentElement.classList.add("dark")}function ie(){const e=document.documentElement.classList.toggle("dark");localStorage.setItem("gtfs-theme",e?"dark":"light"),dt()}function dt(){const e=document.documentElement.classList.contains("dark");document.querySelectorAll(".dark-toggle").forEach(t=>{t.textContent=e?"☀":"☾",t.title=e?"Açık mod":"Koyu mod"})}function ce(){const e=document.documentElement.classList.contains("dark");return`<button class="btn btn-ghost dark-toggle" title="${e?"Açık mod":"Koyu mod"}">${e?"☀":"☾"}</button>`}function H(){const e=Z(),t=document.getElementById("app");if(e.page==="upload"||!e.result){t.innerHTML=`
      <header class="app-header">
        <h1>GTFS Analyzer</h1>
        <span style="flex:1"></span>
        ${ce()}
      </header>
      <main id="page-root"></main>`,t.querySelector(".dark-toggle").addEventListener("click",ie),we(document.getElementById("page-root"));return}const s=Object.keys(ne).map(d=>`
      <button class="nav-btn ${e.page===d?"active":""}" data-page="${d}">
        ${ne[d]}
      </button>`).join("");t.innerHTML=`
    <header class="app-header">
      <h1>GTFS Validator</h1>
      <span class="header-filename">${pt(e.fileName)}</span>
      <button id="btn-back" class="btn btn-ghost">← Ana Sayfa</button>
      <button id="btn-new" class="btn btn-secondary">Yeni GTFS Yükle</button>
      ${ce()}
    </header>
    <nav class="app-nav">${s}</nav>
    <main id="page-root"></main>`,t.querySelector("#btn-back").addEventListener("click",()=>{D("upload"),H()}),t.querySelector("#btn-new").addEventListener("click",()=>{D("upload"),H()}),t.querySelector(".dark-toggle").addEventListener("click",ie),t.querySelectorAll(".nav-btn").forEach(d=>{d.addEventListener("click",()=>{D(d.dataset.page),H()})});const o=document.getElementById("page-root");switch(e.page){case"domain":Fe(o,e.result);break;case"fix":je(o,e.result),Qe(o,e.result,e.result.capped_totals);break;case"rules":Xe(o,e.result);break;case"export":at(o,e.result,e.fileName);break}}function pt(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}ct();H();
