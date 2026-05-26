(function(){const t=document.createElement("link").relList;if(t&&t.supports&&t.supports("modulepreload"))return;for(const c of document.querySelectorAll('link[rel="modulepreload"]'))o(c);new MutationObserver(c=>{for(const f of c)if(f.type==="childList")for(const p of f.addedNodes)p.tagName==="LINK"&&p.rel==="modulepreload"&&o(p)}).observe(document,{childList:!0,subtree:!0});function s(c){const f={};return c.integrity&&(f.integrity=c.integrity),c.referrerPolicy&&(f.referrerPolicy=c.referrerPolicy),c.crossOrigin==="use-credentials"?f.credentials="include":c.crossOrigin==="anonymous"?f.credentials="omit":f.credentials="same-origin",f}function o(c){if(c.ep)return;c.ep=!0;const f=s(c);fetch(c.href,f)}})();const G={page:"upload",result:null,configDelta:sessionStorage.getItem("gtfs-config-delta")??"",fileName:""};function Z(){return G}function me(e,t){G.result=e,G.fileName=t,G.page="domain"}function D(e){G.page=e}function de(e){G.configDelta=e,sessionStorage.setItem("gtfs-config-delta",e)}const O={CRITICAL:"KRİTİK",HIGH:"YÜKSEK",MEDIUM:"ORTA",LOW:"DÜŞÜK",INFO:"BİLGİ"},U={SPEC:"GTFS Geçerliliği",INTEROP:"GTFS Uyumluluğu",QUALITY:"GTFS Kalitesi",ANALYTICS:"GTFS Analitiği"},_e={ZipUnreadable:"ZIP dosyası açılamadı",Utf8Critical:"UTF-8 kodlama hatası",NoRequiredFiles:"Zorunlu dosyalar eksik",CsvMalformed:"CSV biçim hatası",ResourceLimit:"Notice limiti aşıldı",InvalidInput:"Geçersiz girdi"},j={CRITICAL:"var(--color-critical)",HIGH:"var(--color-high)",MEDIUM:"var(--color-medium)",LOW:"var(--color-low)",INFO:"var(--color-info)"},W=new Worker(new URL(""+new URL("validator-worker-CvUGce2e.js",import.meta.url).href,import.meta.url),{type:"module"}),K=new Map;let ye=1;W.onmessage=e=>{const t=e.data,s=K.get(t.id);if(s){if(t.type==="file-list"){s.callbacks?.onFileList?.(t.files);return}if(t.type==="file-done"){s.callbacks?.onFileDone?.(t.name,t.rows,t.bytes);return}if(t.type==="stage"){s.callbacks?.onStageDone?.(t.stage,t.elapsed_ms);return}t.type==="result"&&(K.delete(t.id),t.ok?s.resolve(t.result):s.reject(t.error))}};W.onerror=e=>{const t={code:"ResourceLimit",message:e.message||"Arka plan doğrulama işçisi beklenmedik şekilde durdu."};for(const[,s]of K)s.reject(t);K.clear()};function ve(e,t,s){const o=ye++;return new Promise((c,f)=>{K.set(o,{resolve:c,reject:f,callbacks:s}),W.postMessage({id:o,type:"validate",buffer:e,configDelta:t},[e])})}const pe={K1:"K1 — Dosya Ayrıştırma",K2:"K2 — Kural Doğrulama",K3:"K3 — Varlık Haritası",K4:"K4 — Çapraz Kontrol",K5:"K5 — Türetilmiş Veri",K6:"K6 — İstatistik Analizi",K7:"K7 — Rapor Oluşturma"},C=["K1","K2","K3","K4","K5","K6","K7"],ke=[{key:"max_speed_bus_kmh",label:"Maks. Otobüs Hızı",unit:"km/s",type:"float",def:120,min:60,max:200,desc:"Otobüs seferleri için maksimum izin verilen hız"},{key:"max_speed_tram_kmh",label:"Maks. Tramvay Hızı",unit:"km/s",type:"float",def:100,min:40,max:160,desc:"Tramvay seferleri için maksimum izin verilen hız"},{key:"max_speed_metro_kmh",label:"Maks. Metro Hızı",unit:"km/s",type:"float",def:150,min:80,max:250,desc:"Metro seferleri için maksimum izin verilen hız"},{key:"max_speed_rail_kmh",label:"Maks. Demiryolu Hızı",unit:"km/s",type:"float",def:300,min:100,max:400,desc:"Demiryolu seferleri için maksimum izin verilen hız"},{key:"max_speed_ferry_kmh",label:"Maks. Feribot Hızı",unit:"km/s",type:"float",def:80,min:20,max:150,desc:"Feribot seferleri için maksimum izin verilen hız"},{key:"max_speed_cablecar_kmh",label:"Maks. Teleferik Hızı",unit:"km/s",type:"float",def:30,min:10,max:60,desc:"Teleferik/füniküler için maksimum izin verilen hız"},{key:"min_transfer_time_sec",label:"Min. Aktarma Süresi",unit:"sn",type:"int",def:180,min:30,max:1800,desc:"Transferler için minimum bağlantı süresi"},{key:"max_transfer_distance_m",label:"Maks. Aktarma Mesafesi",unit:"m",type:"float",def:500,min:50,max:2e3,desc:"Transfer geçerli sayılmak için maksimum mesafe"},{key:"max_shape_jump_km",label:"Maks. Güzergah Sıçraması",unit:"km",type:"float",def:10,min:1,max:50,desc:"Art arda güzergah noktaları arasındaki maksimum mesafe"},{key:"stop_too_close_m",label:"Çok Yakın Durak Eşiği",unit:"m",type:"float",def:5,min:1,max:20,desc:"Bu mesafeden yakın duraklar tekrar sayılır"},{key:"stop_far_from_shape_m",label:"Durağın Güzergah'a Uzaklığı",unit:"m",type:"float",def:100,min:20,max:500,desc:"Durağın güzergahından en fazla bu kadar uzakta olabilir"},{key:"stop_far_from_parent_m",label:"Üst İstasyona Uzaklık Eşiği",unit:"m",type:"float",def:100,min:10,max:1e3,desc:"Durak, üst istasyonundan en fazla bu kadar uzakta olabilir"},{key:"feed_expiry_warning_days",label:"Son Kullanma Uyarısı",unit:"gün",type:"int",def:30,min:1,max:60,desc:"Feed bu kadar günden az kalmışsa uyarı üretilir"},{key:"service_gap_days",label:"Servis Boşluğu Eşiği",unit:"gün",type:"int",def:7,min:3,max:30,desc:"Bu günden uzun servis kesintisi işaretlenir"},{key:"max_trip_duration_hours",label:"Maks. Sefer Süresi",unit:"saat",type:"float",def:24,min:8,max:72,desc:"Tek bir seferin maksimum süresi"},{key:"min_trip_duration_sec",label:"Min. Sefer Süresi",unit:"sn",type:"int",def:60,min:10,max:300,desc:"Tek bir seferin minimum süresi"},{key:"max_headway_warning_min",label:"Maks. Sefer Aralığı",unit:"dk",type:"int",def:240,min:60,max:720,desc:"Bu dakikadan uzun aralık uyarı üretir"},{key:"bunching_threshold_min",label:"Sıkışma Eşiği",unit:"dk",type:"int",def:2,min:1,max:10,desc:"Bu dakikadan kısa aralık sıkışma sayılır"}],ue={"stops.txt":"durak","routes.txt":"hat","trips.txt":"sefer","stop_times.txt":"geçiş zamanı","shapes.txt":"güzergah","agency.txt":"işletici","calendar.txt":"takvim","calendar_dates.txt":"özel gün","frequencies.txt":"frekans","transfers.txt":"aktarma","pathways.txt":"geçit","levels.txt":"kat","attributions.txt":"atıf","translations.txt":"çeviri"};function Se(e){const t=Z(),s=t.result!==null,o=s?"Farklı GTFS Yükle":"GTFS Yükle";e.innerHTML=`
    <div class="up-grid">

      <!-- Sol üst: GTFS Dosyaları -->
      <div class="up-cell up-files">
        <div class="lp-section-title">GTFS Dosyaları</div>
        <div id="file-rows" class="lp-rows">
          ${s&&t.result.metrics.file_stats.length>0?fe(t.result.metrics.file_stats):'<div class="lp-placeholder">Henüz dosya yüklenmedi</div>'}
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
          ${C.map(c=>`
            <div class="lp-stage-row ${s?"done":"waiting"}" data-stage="${c}">
              <span class="lp-icon">${s?"✓":"○"}</span>
              <span class="lp-label">${P(pe[c]??c)}</span>
              <span class="lp-status">${s?"":"—"}</span>
            </div>`).join("")}
        </div>
      </div>

      <!-- Sağ alt: Kalite Skoru + Rapor butonu -->
      <div class="up-cell up-score">
        <div id="score-panel" class="score-compact${s?"":" hidden"}">
          ${s?he(t.result):""}
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
          ${$e(Te(t.configDelta))}
        </div>
        <div class="settings-actions">
          <button id="btn-apply-settings" class="btn btn-primary btn-sm">Uygula</button>
          <button id="btn-reset-settings" class="btn btn-ghost btn-sm">Sıfırla</button>
          <span id="settings-status" class="settings-status hidden"></span>
        </div>
      </div>
    </div>`,we(e)}function fe(e){return e.map(t=>{const s=Math.round(t.bytes/1024),o=ue[t.name]??"satır";return`
      <div class="lp-file-row done">
        <span class="lp-icon">✓</span>
        <span class="lp-label">${P(t.name)}</span>
        <span class="lp-status">${t.rows.toLocaleString("tr-TR")} ${o} · ${V(s)}</span>
      </div>`}).join("")}function he(e){const{r5:t}=e.reports,s=[{label:"GTFS Geçerliliği",weight:"×40%",value:t.spec_score,color:"#2563eb",bg:"#eff6ff"},{label:"GTFS Uyumluluğu",weight:"×30%",value:t.interop_score,color:"#92400e",bg:"#fef3c7"},{label:"GTFS Kalitesi",weight:"×20%",value:t.quality_score,color:"#15803d",bg:"#f0fdf4"},{label:"GTFS Analitiği",weight:"×10%",value:t.analytics_score,color:"#6d28d9",bg:"#f5f3ff"}],o=t.score>=80?"#15803d":t.score>=60?"#92400e":"#dc2626",c=t.pub_score>=80?"#15803d":t.pub_score>=60?"#d97706":"#dc2626",f=s.map(p=>`<div class="sc-chip" style="background:${p.bg};color:${p.color}">
      <div class="sc-chip-left">
        <span class="sc-chip-label">${p.label}</span>
        <span class="sc-weight">${p.weight}</span>
      </div>
      <span class="sc-chip-val">${p.value.toFixed(1)}</span>
    </div>`).join("");return`
    <div class="sc-dual-header">
      <div class="sc-dual-item">
        <span class="sc-title">Yayın Skoru</span>
        <span class="sc-overall" style="color:${c}">${t.pub_score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
      <div class="sc-dual-sep"></div>
      <div class="sc-dual-item">
        <span class="sc-title">Kalite Skoru</span>
        <span class="sc-overall" style="color:${o}">${t.score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
    </div>
    <div class="sc-chips">${f}</div>`}function $e(e){return ke.map(t=>{const s=e[t.key]!==void 0?e[t.key]:t.def,o=e[t.key]!==void 0;return`
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
      </div>`}).join("")}function Te(e){if(!e||e==="{}")return{};try{return JSON.parse(e)}catch{return{}}}function we(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),o=e.querySelector("#upload-error");s.addEventListener("change",()=>{const h=s.files?.[0];h&&se(h,e,o)}),t.addEventListener("dragover",h=>{h.preventDefault(),t.classList.contains("loading")||t.classList.add("drag-over")}),t.addEventListener("dragleave",()=>t.classList.remove("drag-over")),t.addEventListener("drop",h=>{if(h.preventDefault(),t.classList.remove("drag-over"),t.classList.contains("loading"))return;const a=h.dataTransfer?.files[0];a&&se(a,e,o)}),t.addEventListener("keydown",h=>{(h.key==="Enter"||h.key===" ")&&!t.classList.contains("loading")&&s.click()}),e.querySelector("#btn-report")?.addEventListener("click",()=>{D("domain"),H()});const c=e.querySelector("#settings-toggle"),f=e.querySelector("#settings-body"),p=e.querySelector("#settings-arrow");c.addEventListener("click",()=>{const h=!f.classList.contains("hidden");f.classList.toggle("hidden",h),p.textContent=h?"▶":"▼"}),e.querySelector("#btn-apply-settings")?.addEventListener("click",()=>Le(e)),e.querySelector("#btn-reset-settings")?.addEventListener("click",()=>Re(e)),e.querySelectorAll(".sf-reset").forEach(h=>{h.addEventListener("click",()=>{const a=h.dataset.key,l=e.querySelector(`.sf-input[data-key="${CSS.escape(a)}"]`);l&&(l.value=l.dataset.def,l.closest(".sf-row")?.classList.remove("sf-overridden"),h.style.visibility="hidden")})}),e.querySelectorAll(".sf-input").forEach(h=>{h.addEventListener("change",()=>{const a=e.querySelector(`.sf-reset[data-key="${CSS.escape(h.dataset.key)}"]`);a&&(a.style.visibility="visible"),h.closest(".sf-row")?.classList.add("sf-overridden")})})}function Le(e){const t={};e.querySelectorAll(".sf-input").forEach(c=>{const f=c.dataset.key,p=parseFloat(c.dataset.def),h=c.dataset.type==="int"?parseInt(c.value,10):parseFloat(c.value);!isNaN(h)&&h!==p&&(t[f]=h)});const s=Object.keys(t).length>0?JSON.stringify(t):"";de(s);const o=e.querySelector("#settings-status");o.textContent="Ayarlar kaydedildi. Yeni ZIP yükleyince uygulanır.",o.className="settings-status ok",setTimeout(()=>{o.className="settings-status hidden"},3e3)}function Re(e){e.querySelectorAll(".sf-input").forEach(s=>{s.value=s.dataset.def,s.closest(".sf-row")?.classList.remove("sf-overridden");const o=e.querySelector(`.sf-reset[data-key="${CSS.escape(s.dataset.key)}"]`);o&&(o.style.visibility="hidden")}),de("");const t=e.querySelector("#settings-status");t.textContent="Varsayılanlara döndürüldü.",t.className="settings-status ok",setTimeout(()=>{t.className="settings-status hidden"},3e3)}async function se(e,t,s){if(!e.name.endsWith(".zip")){le(s,{code:"InvalidInput",message:"Yalnızca .zip uzantılı dosyalar kabul edilir."});return}s.className="upload-status hidden",ze(t,e.name,e.size);try{const o=await e.arrayBuffer(),c=await ve(o,Z().configDelta,{onFileList:f=>{const p=t.querySelector("#file-rows");p&&(p.innerHTML="",f.forEach(h=>{const a=document.createElement("div");a.className="lp-file-row reading",a.dataset.file=h.name;const l=Math.round(h.uncompressed_size/1024);a.innerHTML=`
            <span class="lp-icon">○</span>
            <span class="lp-label">${P(h.name)}</span>
            <span class="lp-status">${V(l)} okunuyor…</span>`,p.appendChild(a)}))},onFileDone:(f,p,h)=>{const l=t.querySelector("#file-rows")?.querySelector(`[data-file="${CSS.escape(f)}"]`);if(!l)return;l.className="lp-file-row done",l.querySelector(".lp-icon").textContent="✓";const r=ue[f]??"satır";l.querySelector(".lp-status").textContent=`${p.toLocaleString("tr-TR")} ${r} · ${V(Math.round(h/1024))}`},onStageDone:(f,p)=>{const h=t.querySelector(`[data-stage="${CSS.escape(f)}"]`);if(!h)return;h.className="lp-stage-row done",h.querySelector(".lp-icon").textContent="✓",h.querySelector(".lp-status").textContent=p<1e3?`${p} ms`:`${(p/1e3).toFixed(1)} sn`;const a=C.indexOf(f);if(a>=0&&a<C.length-1){const l=t.querySelector(`[data-stage="${CSS.escape(C[a+1])}"]`);l?.classList.contains("waiting")&&(l.className="lp-stage-row running",l.querySelector(".lp-icon").textContent="⋯",l.querySelector(".lp-status").textContent="çalışıyor…")}}});me(c,e.name),Pe(t,c)}catch(o){ge(t),le(s,o)}}function ze(e,t,s){const o=e.querySelector("#drop-zone"),c=e.querySelector("#file-input"),f=e.querySelector("#upload-btn-label"),p=e.querySelector("#score-panel");p&&(p.classList.add("hidden"),p.innerHTML="");const h=e.querySelector("#btn-report");h&&(h.disabled=!0),o.classList.add("loading"),c.disabled=!0,f.setAttribute("aria-disabled","true"),f.style.opacity="0.5",f.style.pointerEvents="none";const a=(s/1048576).toFixed(1),l=o.querySelector(".drop-primary");l&&(l.textContent=`${t} (${a} MB) işleniyor…`);const r=e.querySelector("#stage-rows");r&&(r.innerHTML=C.map((n,i)=>`
      <div class="lp-stage-row ${i===0?"running":"waiting"}" data-stage="${n}">
        <span class="lp-icon">${i===0?"⋯":"○"}</span>
        <span class="lp-label">${P(pe[n]??n)}</span>
        <span class="lp-status">${i===0?"çalışıyor…":"bekliyor"}</span>
      </div>`).join(""))}function ge(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),o=e.querySelector("#upload-btn-label");t.classList.remove("loading"),s.disabled=!1,o.removeAttribute("aria-disabled"),o.style.opacity="",o.style.pointerEvents=""}function Pe(e,t){ge(e);const s=e.querySelector("#upload-btn-label");s.textContent="Farklı GTFS Yükle";const o=e.querySelector("#score-panel");o.className="score-compact",o.innerHTML=he(t);const c=e.querySelector("#btn-report");c.disabled=!1,c.addEventListener("click",()=>{D("domain"),H()},{once:!0});const f=e.querySelector("#file-rows");f&&t.metrics.file_stats.length>0&&(f.innerHTML=fe(t.metrics.file_stats))}function le(e,t){const s=_e[t.code]??t.code;e.className="upload-status error",e.innerHTML=`<strong>${P(s)}</strong><p>${P(t.message)}</p>`}function V(e){return e>=1024?`${(e/1024).toFixed(1)} MB`:`${e} KB`}function P(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function Me(e,t){const{r1:s,r5:o}=t.reports,c=t.metrics,f=He(t);e.innerHTML=`
    <div class="report-page">
      ${Ae(o,f)}
      ${Ee(s,t)}
      ${Fe(o)}
      ${Oe(c)}
    </div>`}function Ae(e,t){const s=ae(e.pub_score),o=ae(e.score);return`
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
      ${Ge(t)}
    </div>`}function ae(e){return e>=80?"#16a34a":e>=60?"#d97706":"#dc2626"}function Ee(e,t){const s=new Map(t.notices.map(o=>[o.id,o]));if(!e.publishable){const o=e.blocker_notice_ids.slice(0,5).map(c=>{const f=s.get(c);return f?`<li><code>${q(f.rule_id)}</code> — ${q(f.title||f.message.slice(0,80))}</li>`:""}).join("");return`
      <div class="rpt-r1 rpt-r1-blocked">
        <div class="rpt-r1-icon">✕</div>
        <div class="rpt-r1-body">
          <strong>Yayınlanması Tavsiye Edilmez</strong>
          <span>${e.blocker_notice_ids.length} kritik engelleyici bulgu mevcut — feed yayına alınamaz.</span>
          ${o?`<ul class="rpt-r1-list">${o}</ul>`:""}
        </div>
      </div>`}if(e.conditional){const o=e.conditional_blocker_notice_ids.slice(0,5).map(c=>{const f=s.get(c);return f?`<li><code>${q(f.rule_id)}</code> — ${q(f.title||f.message.slice(0,80))}</li>`:""}).join("");return`
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
    </div>`}function Fe(e){return`
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
    </div>`}const oe=[{key:"CRITICAL",label:"Kritik",color:"#dc2626"},{key:"HIGH",label:"Yüksek",color:"#d97706"},{key:"MEDIUM",label:"Orta",color:"#2563eb"},{key:"LOW",label:"Düşük",color:"#16a34a"},{key:"INFO",label:"Bilgi",color:"#9ca3af"}];function Ge(e){const t=e.CRITICAL+e.HIGH+e.MEDIUM+e.LOW+e.INFO;if(t===0)return"";const s=15.9155;let o=0;const c=oe.filter(p=>e[p.key]>0).map(p=>{const h=e[p.key]/t*100,a=25-o;return o+=h,`<circle cx="18" cy="18" r="${s}" fill="none" stroke="${p.color}" stroke-width="3.5" stroke-dasharray="${h.toFixed(3)} ${(100-h).toFixed(3)}" stroke-dashoffset="${a.toFixed(3)}"/>`}).join(""),f=oe.filter(p=>e[p.key]>0).map(p=>{const h=e[p.key],a=(h/t*100).toFixed(1);return`<div class="pie-leg-row"><span class="pie-leg-dot" style="background:${p.color}"></span><span class="pie-leg-label">${p.label}</span><span class="pie-leg-count">${h.toLocaleString("tr-TR")}</span><span class="pie-leg-pct">${a}%</span></div>`}).join("");return`
    <div class="rpt-score-card rpt-pie-card">
      <span class="rpt-score-label">Hata Dağılımı <span class="rpt-score-total">${t.toLocaleString("tr-TR")}</span></span>
      <div class="rpt-pie-body">
        <svg viewBox="0 0 36 36" class="rpt-pie-svg" aria-hidden="true">
          <circle cx="18" cy="18" r="${s}" fill="none" class="pie-track" stroke-width="3.5"/>
          ${c}
          <text x="18" y="16" text-anchor="middle" dominant-baseline="middle"
            font-size="5.5" font-weight="800" class="pie-center-num">${t.toLocaleString("tr-TR")}</text>
          <text x="18" y="22" text-anchor="middle" dominant-baseline="middle"
            font-size="2.8" class="pie-center-txt">hata</text>
        </svg>
        <div class="rpt-pie-legend">${f}</div>
      </div>
    </div>`}function He(e){const t={CRITICAL:0,HIGH:0,MEDIUM:0,LOW:0,INFO:0};for(const s of e.notices)s.severity in t&&t[s.severity]++;return t}function Oe(e){const s=[{label:"Durak",value:e.stop_count.toLocaleString("tr-TR")},{label:"Hat",value:e.route_count.toLocaleString("tr-TR")},{label:"Sefer",value:e.trip_count.toLocaleString("tr-TR")},{label:"Güzergah",value:e.shape_count.toLocaleString("tr-TR")},{label:"Aktif Servis Günü",value:e.active_service_days.toLocaleString("tr-TR")},{label:"Ort. Günlük Sefer",value:e.avg_daily_trips.toFixed(0)}].map(c=>`
    <div class="rpt-metric">
      <span class="rpt-metric-val">${c.value}</span>
      <span class="rpt-metric-label">${c.label}</span>
    </div>`).join(""),o=e.file_stats.map(c=>`
    <tr>
      <td><code>${q(c.name)}</code></td>
      <td class="num">${c.rows.toLocaleString("tr-TR")}</td>
      <td class="num">${qe(c.bytes)}</td>
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
    </div>`}function qe(e){return e<1024?`${e} B`:e<1024*1024?`${(e/1024).toFixed(1)} KB`:`${(e/1024/1024).toFixed(1)} MB`}function q(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}let $=null,S=null;function De(e,t){Ke(),$.querySelector(".mm-title").textContent=e,$.classList.remove("mm-hidden");const s=$.querySelector(".mm-map");S?S.eachLayer(p=>{p instanceof L.TileLayer||S.removeLayer(p)}):(S=L.map(s),L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",{attribution:"© OpenStreetMap katkıcıları",maxZoom:19}).addTo(S));const o=[];if(t.polyline&&t.polyline.length>1){L.polyline(t.polyline,{color:"#f59e0b",weight:3,opacity:.8}).addTo(S);for(const p of t.polyline)o.push(p);if(t.showArrows)for(const p of Ce(t.polyline,250))L.marker([p.lat,p.lon],{icon:L.divIcon({html:`<svg width="14" height="14" viewBox="-7 -7 14 14" style="transform:rotate(${p.angle}deg);display:block">
              <polygon points="0,-6 5,4 -5,4" fill="#f59e0b" stroke="#fff" stroke-width="1.5"/>
            </svg>`,className:"",iconSize:[14,14],iconAnchor:[7,7]}),interactive:!1}).addTo(S)}let c=null;if(t.extraPolylines){for(const p of t.extraPolylines)if(p.coords.length>1){L.polyline(p.coords,{color:p.color,weight:p.weight??4,opacity:.95}).addTo(S);for(const h of p.coords)o.push(h);p.zoomTo&&(c=p.coords)}}for(const p of t.pins){const h=p.primary?"#2563eb":"#dc2626",a=p.small?5:9,l=p.small?1.5:2.5;L.circleMarker([p.lat,p.lon],{radius:a,fillColor:h,color:"#fff",weight:l,fillOpacity:.85}).addTo(S).bindPopup(p.label,{maxWidth:260}),o.push([p.lat,p.lon])}c&&c.length>1?S.fitBounds(c,{padding:[60,60],maxZoom:16}):o.length===0?S.setView([39,35],6):o.length===1?S.setView(o[0],16):S.fitBounds(o,{padding:[40,40]});const f=$.querySelector(".mm-legend");t.legendItems&&t.legendItems.length>0?(f.innerHTML=t.legendItems.map(p=>`<div class="mm-legend-item"><div class="mm-dot" style="background:${p.color}"></div>${p.label}</div>`).join(""),f.style.display="flex"):f.style.display="none",setTimeout(()=>S.invalidateSize(),50)}function Y(){$?.classList.add("mm-hidden")}function Ce(e,t){const o=l=>l*Math.PI/180,c=(l,r,n,i)=>{const d=o(n-l),u=o(i-r),g=Math.sin(d/2)**2+Math.cos(o(l))*Math.cos(o(n))*Math.sin(u/2)**2;return 12742e3*Math.atan2(Math.sqrt(g),Math.sqrt(1-g))},f=(l,r,n,i)=>{const d=o(i-r),u=Math.sin(d)*Math.cos(o(n)),g=Math.cos(o(l))*Math.sin(o(n))-Math.sin(o(l))*Math.cos(o(n))*Math.cos(d);return(Math.atan2(u,g)*180/Math.PI+360)%360},p=[];let h=0,a=t*.5;for(let l=0;l<e.length-1;l++){const[r,n]=e[l],[i,d]=e[l+1],u=c(r,n,i,d),g=f(r,n,i,d);for(;h+u>=a;){const b=(a-h)/u;p.push({lat:r+b*(i-r),lon:n+b*(d-n),angle:g}),a+=t}h+=u}return p}function Ke(){$||($=document.createElement("div"),$.className="mm-overlay mm-hidden",$.innerHTML=`
    <div class="mm-box" role="dialog" aria-modal="true" aria-label="Harita">
      <div class="mm-header">
        <span class="mm-title"></span>
        <button class="mm-close" title="Kapat" aria-label="Kapat">✕</button>
      </div>
      <div class="mm-map"></div>
      <div class="mm-legend"></div>
    </div>`,document.body.appendChild($),$.querySelector(".mm-close").addEventListener("click",Y),$.addEventListener("click",e=>{e.target===$&&Y()}),document.addEventListener("keydown",e=>{e.key==="Escape"&&Y()}))}const xe={blocker:"Engelleyici",conditional_blocker:"Koşullu Engelleyici",interop:"Uyumluluk",propagation:"Zincirleme","quick-win":"Kolay Düzeltme",quality:"Kalite Öncelikli",widespread:"Yaygın",analytics:"Analitik",hard:"Zor",single:"Tek Örnek","high-impact":"Yüksek Etki"};function Be(e,t){const s=new Map(t.notices.map(a=>[a.id,a])),o=t.reports.r9.items.reduce((a,l)=>a+l.score_delta,0),c=o>0?(100-t.reports.r5.score)/o:1,f=t.reports.r9.items.reduce((a,l)=>a+l.pub_score_delta,0),p=f>0?(100-t.reports.r5.pub_score)/f:1,h=new Map(t.reports.r9.items.map(a=>[a.rule_id,{qualDelta:a.score_delta*c,pubDelta:a.pub_score_delta*p,count:a.affected_instance_count}]));e.innerHTML=`
    <section class="page-fix">
      ${Ie(t.reports.r9.items,s,c,p,t.capped_totals)}
      ${Ne(t,s,h,t.name_index)}
    </section>`}function Ie(e,t,s,o,c){if(e.length===0)return'<div class="card"><h2>Düzeltme Kuyruğu (R9)</h2><p class="empty">Düzeltilecek bulgu yok.</p></div>';const f=e.map((p,h)=>{const a=p.notice_ids[0]?t.get(p.notice_ids[0]):void 0,l=a?.severity??"INFO",r=p.labels.map(y=>`<span class="label-badge label-${y}">${xe[y]??y}</span>`).join(""),n=p.pub_score_delta*o,i=p.score_delta*s,d=n>=.05?`<span class="pub-delta">+${n.toFixed(1)}</span>`:'<span class="muted-text">—</span>',u=i>=.05?`<span class="score-delta">+${i.toFixed(1)}</span>`:'<span class="muted-text">—</span>',g=c[p.rule_id],b=g!=null?`<span class="cap-total">${g.toLocaleString("tr-TR")}</span>`:'<span class="muted-text">—</span>',m=`
      <tr class="r9-main-row" data-idx="${h}">
        <td>
          <span class="r9-arrow">▶</span> <code>${T(p.rule_id)}</code>
          ${a?.title?`<span class="r9-rule-title">${T(a.title)}</span>`:""}
        </td>
        <td style="color:${j[l]}">${O[l]}</td>
        <td>${r}</td>
        <td>${p.affected_instance_count}</td>
        <td>${b}</td>
        <td class="score">${p.priority_score.toFixed(1)}</td>
        <td class="score-delta-cell">${d}</td>
        <td class="score-delta-cell">${u}</td>
        <td>${p.realized_dependent_count}</td>
        <td>${p.fix_effort.toFixed(1)}</td>
      </tr>`,_=`
      <tr class="r9-detail-row" data-for="${h}" hidden>
        <td colspan="10">
          <div class="r9-detail">
            ${a?.title?`<p><strong>Mesaj:</strong> ${T(a.title)}</p>`:""}
            ${a?.remediation?`<p><strong>Çözüm:</strong> ${T(a.remediation)}</p>`:""}
          </div>
        </td>
      </tr>`;return m+_}).join("");return`
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
            <th>Toplam <span class="col-info" title="Kural başına notice sınırı (cap) uygulandığında gerçek ihlal sayısı. Raporda yalnızca sınır kadar notice gösterilmektedir; sınıra çarpmayan kurallarda bu sütun boş kalır.">ℹ</span></th>
            <th class="score">Skor <span class="col-info" title="Öncelik puanı. Ciddiyet × (1+Bağımlı) × log₂(1+Etkilenen) / Çaba formülüyle hesaplanır. Yüksek skor = önce düzelt.">ℹ</span></th>
            <th class="score-delta-cell">+Yayın <span class="col-info" title="Bu kural düzeltilirse yayın skoru kaç puan artar. Yalnızca blocker/conditional_blocker kurallar için gösterilir. Toplamı = 100 − mevcut yayın skoru.">ℹ</span></th>
            <th class="score-delta-cell">+Kalite <span class="col-info" title="Bu kural düzeltilirse kalite skoru kaç puan artar. Toplamı = 100 − mevcut kalite skoru.">ℹ</span></th>
            <th>Bağımlı <span class="col-info" title="Bu kural düzeltilince kaç başka kural daha kendiliğinden kapanır (bu feed'de ateşlenmiş olan bağımlı kurallar).">ℹ</span></th>
            <th>Çaba <span class="col-info" title="Düzeltmenin iş yükü: 1=kolay (tek alan değişikliği), 5=zor (veri modelinde kapsamlı revizyon).">ℹ</span></th>
          </tr></thead>
          <tbody>${f}</tbody>
        </table>
      </div>
    </div>`}function Ne(e,t,s,o){const c=e.reports.r2.items;if(c.length===0)return'<div class="card"><h2>Tüm Bulgular (R2)</h2><p class="empty">Bulgu yok.</p></div>';const h=`
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
          ${[...new Map(c.filter(l=>t.has(l.notice_id)).map(l=>{const r=t.get(l.notice_id);return[r.rule_id,r.title||r.rule_id]})).entries()].sort(([l],[r])=>l.localeCompare(r)).map(([l,r])=>`<option value="${T(l)}">${T(l)} — ${T(r)}</option>`).join("")}
        </select>
      </label>
      <span id="filter-count" class="filter-count"></span>
    </div>`,a=c.map(l=>{const r=t.get(l.notice_id);if(!r)return"";const n=s.get(r.rule_id),i=n!=null&&n.count>0?n.pubDelta/n.count:null,d=n!=null&&n.count>0?n.qualDelta/n.count:null,u=i!=null&&i>=.005?`<span class="pub-delta">+${i.toFixed(2)}</span>`:'<span class="muted-text">—</span>',g=d!=null&&d>=.005?`<span class="score-delta">+${d.toFixed(2)}</span>`:'<span class="muted-text">—</span>',b=Ue(r,o)?`<button class="map-pin-btn" data-notice-id="${T(r.id)}" title="Haritada göster" aria-label="Haritada göster">📍</button>`:"";return`
      <tr data-severity="${r.severity}" data-class="${r.rule_class}" data-rule="${r.rule_id}">
        <td>${T(l.display_label)}</td>
        <td style="color:${j[r.severity]}">${O[r.severity]}</td>
        <td>${U[r.rule_class]}</td>
        <td>${T(r.message)}</td>
        <td>${r.file?T(r.file):"—"}</td>
        <td>${r.line??"—"}</td>
        <td>${r.field?T(r.field):"—"}</td>
        <td class="score-delta-cell">${u}</td>
        <td class="score-delta-cell">${g}</td>
        <td class="map-btn-cell">${b}</td>
      </tr>`}).join("");return`
    <div class="card" id="r2-card">
      <h2>Tüm Bulgular (R2) <span class="count-badge">${c.length}</span></h2>
      ${h}
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
    </div>`}function T(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function v(e,t,s,o){const c=t.stop_coords[e];if(!c)return null;const p=`<strong>${t.stops[e]??e}</strong>${o?` ${o}`:""}<br><code>${e}</code>`;return{lat:c[0],lon:c[1],label:p,primary:s}}const je=new Set(["STP_003","STP_004","STP_005","STP_006","STP_007","STP_008","STP_009","STP_010","STP_011","STP_012","STP_013","STP_014","STP_015","STP_016","STP_017","STP_018","STP_019","STP_020","STP_021","STP_022","STP_023","STP_024","STP_025","STP_026","STP_027","STP_028","STP_029","STP_030","GEO_001","GEO_002","GEO_003","GEO_004","GEO_005","GEO_009","GEO_010","GEO_011","GEO_012","GEO_014","PTH_012","PTH_013","PTH_015","PTH_016","PTH_017","PTH_018","PTH_019","LVL_006","SHP_022","VAT_007"]),Q=new Set(["TRP_001","TRP_002","TRP_003","TRP_004","TRP_005","TRP_006","TRP_007","TRP_008","TRP_009","TRP_010","TRP_011","TRP_012","TRP_013","TRP_014","TRP_015","TRP_016","TRP_017","TRP_018","TRP_019","TRP_020","FRQ_001","FRQ_002","FRQ_003","FRQ_004","FRQ_005","FRQ_006","FRQ_007","FRQ_008","FRQ_009","STM_001","STM_002","STM_003","STM_004","STM_005","STM_006","STM_007","STM_008","STM_009","STM_010","STM_011","STM_013","STM_015","STM_016","STM_018","STM_019","STM_021","STM_022","STM_023","STM_024","STM_027","STM_028","STM_029","STM_030","STM_031","STM_032","STM_034","OPR_006","OPR_017"]),J=new Set(["RTS_001","RTS_002","RTS_003","RTS_004","RTS_005","RTS_006","RTS_007","RTS_008","RTS_009","RTS_010","RTS_011","RTS_012","RTS_013","RTS_014","RTS_015","RTS_016","RTS_017","OPR_001","OPR_002","OPR_003"]);function Ue(e,t){const s=e.entity_id??"";if(je.has(e.rule_id))return!!(s&&s in t.stop_coords);if(e.rule_id==="GEO_013")return Object.keys(t.stop_coords).length>0;if(e.rule_id==="SHP_017"){const o=e.message.match(/\(kod: '([^']+)'\)/)?.[1],c=e.details?.ctx_b??"",f=e.details?.ctx_a??"";return!!(o&&o in t.stop_coords)||c.length>0||f.length>0}if(e.rule_id==="STM_014"){const o=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/);return o&&(o[1]in t.stop_coords||o[2]in t.stop_coords)?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_020"){const o=e.details?.stop_a??"",c=e.details?.stop_b??"";return o&&o in t.stop_coords||c&&c in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_025"){const o=e.details?.stop_a??"",c=e.details?.stop_b??"";return o&&o in t.stop_coords||c&&c in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="OPR_015"){const o=e.details?.shape_id??"";return!!(o&&o in t.shape_coords)}if(e.rule_id==="PTH_014"){const o=e.details?.from_station??"",c=e.details?.to_station??"";return!!(o&&o in t.stop_coords&&c&&c in t.stop_coords)}return["OPR_007","OPR_008","STM_017"].includes(e.rule_id)?!!(s&&s in t.trip_shapes):["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"].includes(e.rule_id)?!!(s&&s in t.shape_coords):["STM_012","STM_026","STM_033"].includes(e.rule_id)?!!(s&&s in t.trip_shapes)||!!(s&&s in t.trip_stops):e.rule_id==="VAT_005"?Object.keys(t.stop_coords).length>0:Q.has(e.rule_id)?!!(s&&(s in t.trip_shapes||s in t.trip_stops)):J.has(e.rule_id)?!!(s&&(t.route_shapes[s]?.length??0)>0):!1}function Ye(e,t){const s=e.entity_id??"";if(e.rule_id==="SHP_017"){const a=e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",l=e.details?.shape_id??"",r=(e.details?.ctx_b??"").split(",").filter(Boolean),n=(e.details?.ctx_a??"").split(",").filter(Boolean),i=(e.details?.seq_b??"").split(",").filter(Boolean),d=(e.details?.seq_a??"").split(",").filter(Boolean),u=e.observed_value??"",g=[];for(let k=0;k<r.length;k++){const w=i[k]?`#${i[k]} `:"",z=v(r[k],t,!0,`${w}(önceki)`);z&&g.push(z)}const b=u?`#${u} `:"",m=a?v(a,t,!1,`${b}⚠ sıra bozuk`):null;m&&g.push(m);for(let k=0;k<n.length;k++){const w=d[k]?`#${d[k]} `:"",z=v(n[k],t,!0,`${w}(sonraki)`);z&&g.push(z)}const _=l?t.shape_coords[l]??[]:[],y=[];return _.length>1&&y.push({color:"#f59e0b",label:"Güzergah şekli"}),y.push({color:"#2563eb",label:"Komşu duraklar"}),y.push({color:"#dc2626",label:"Sırası bozuk durak"}),{pins:g,polyline:_.length>1?_:void 0,legendItems:y,showArrows:_.length>1}}if(e.rule_id==="STM_014"){const a=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/),l=a?v(a[1].trim(),t,!0,"(kalkış)"):null,r=a?v(a[2].trim(),t,!1,"(varış)"):null,n=[l,r].filter(g=>g!==null),i=s?t.trip_shapes[s]??"":"",d=i?t.shape_coords[i]??[]:[],u=[];return d.length>1&&u.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>1&&(u.push({color:"#2563eb",label:"Kalkış durağı"}),u.push({color:"#dc2626",label:"Varış durağı"})),{pins:n,polyline:d.length>1?d:void 0,legendItems:u,showArrows:d.length>1}}if(e.rule_id==="STM_020"){const a=e.details?.stop_a?v(e.details.stop_a,t,!0,"(kalkış)"):null,l=e.details?.stop_b?v(e.details.stop_b,t,!1,"(varış)"):null,r=[a,l].filter(u=>u!==null),n=s?t.trip_shapes[s]??"":"",i=n?t.shape_coords[n]??[]:[],d=[];return i.length>1&&d.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&d.push({color:"#2563eb",label:"Kalkış durağı"}),r.length>1&&d.push({color:"#dc2626",label:"Varış durağı"}),{pins:r,polyline:i.length>1?i:void 0,legendItems:d,showArrows:!0}}if(e.rule_id==="SHP_010"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/^\(([^,]+),([^)]+)\)$/),r=[];if(l){const i=parseFloat(l[1]),d=parseFloat(l[2]);!isNaN(i)&&!isNaN(d)&&r.push({lat:i,lon:d,label:`Tekrarlanan koordinat<br><code>(${l[1]}, ${l[2]})</code>`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&n.push({color:"#dc2626",label:"Tekrarlanan nokta"}),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_022"){const a=e.message.match(/durağı '([^']+)' güzergah şeklinin/)?.[1]??"",l=v(s,t,!1,"⚠ belirsiz eşleşme"),r=a?t.shape_coords[a]??[]:[],n=l?[l]:[],i=[];return r.length>1&&i.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&i.push({color:"#dc2626",label:"Belirsiz durak"}),{pins:n,polyline:r.length>1?r:void 0,legendItems:i,showArrows:!0}}if(e.rule_id==="SHP_018"||e.rule_id==="SHP_019"){const a=s?t.shape_coords[s]??[]:[],l=a.length>1?[{color:"#f59e0b",label:"Kullanılmayan güzergah şekli"}]:[];return{pins:[],polyline:a.length>1?a:void 0,legendItems:l,showArrows:!0}}if(e.rule_id==="OPR_007"){const a=e.details?.dup_stop??e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",l=(e.details?.stops??"").split(",").filter(Boolean),r=s?t.trip_shapes[s]??"":"",n=r?t.shape_coords[r]??[]:[],i=new Set,d=[];for(const g of l){if(i.has(g))continue;i.add(g);const b=g===a,m=v(g,t,!b,b?"(tekrar eden)":void 0);m&&(m.small=!b,d.push(m))}const u=[];return n.length>1&&u.push({color:"#f59e0b",label:"Güzergah şekli"}),d.length>1&&u.push({color:"#2563eb",label:"Duraklar"}),a&&u.push({color:"#dc2626",label:"Tekrar eden durak"}),{pins:d,polyline:n.length>1?n:void 0,legendItems:u,showArrows:!0}}if(e.rule_id==="STM_017"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(d=>v(d,t,!0)).filter(d=>d!==null).map(d=>({...d,small:!0})),i=[];return l.length>1&&i.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&i.push({color:"#2563eb",label:"Sefer durakları"}),{pins:n,polyline:l.length>1?l:void 0,legendItems:i,showArrows:!0}}if(e.rule_id==="GEO_006"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/segment\s+(\d+)→(\d+)/),r=[];if(l&&a.length>0){const i=parseInt(l[1],10),d=parseInt(l[2],10),u=a[i],g=a[d];u&&r.push({lat:u[0],lon:u[1],label:`<strong>Atlama başlangıcı</strong><br>Segment ${i}`,primary:!0}),g&&r.push({lat:g[0],lon:g[1],label:`<strong>Atlama bitişi</strong><br>Segment ${d}`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&(n.push({color:"#dc2626",label:"Atlama başlangıcı"}),n.push({color:"#2563eb",label:"Atlama bitişi"})),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!1}}if(e.rule_id==="SHP_020"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/\(([^,]+),([^)]+)\)$/),r=[];if(l){const i=parseFloat(l[1]),d=parseFloat(l[2]);!isNaN(i)&&!isNaN(d)&&r.push({lat:i,lon:d,label:`Tekrarlayan koordinat<br><code>(${l[1]}, ${l[2]})</code>`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&n.push({color:"#dc2626",label:"Tekrarlayan nokta"}),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_009"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/segment (\d+)-(\d+) ∩ (\d+)-(\d+)/),r=[];if(l&&a.length>0){const i=parseInt(l[1],10),d=parseInt(l[2],10),u=parseInt(l[3],10),g=parseInt(l[4],10),b=a[i],m=a[d],_=a[u],y=a[g];b&&r.push({lat:b[0],lon:b[1],label:`Segment ${i}→${d} başlangıcı`,primary:!0}),m&&r.push({lat:m[0],lon:m[1],label:`Segment ${i}→${d} bitişi`,primary:!0}),_&&r.push({lat:_[0],lon:_[1],label:`Segment ${u}→${g} başlangıcı`,primary:!1}),y&&r.push({lat:y[0],lon:y[1],label:`Segment ${u}→${g} bitişi`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&(n.push({color:"#2563eb",label:"1. kesişen segment"}),n.push({color:"#dc2626",label:"2. kesişen segment"})),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="OPR_008"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=[];l.length>1&&r.push({color:"#9ca3af",label:"Güzergah şekli"});const n=[],i=[],d=new Set;for(let u=0;;u++){const g=e.details?.[`bad_seg_${u}_a`],b=e.details?.[`bad_seg_${u}_b`];if(!g||!b)break;const m=t.stop_coords[g],_=t.stop_coords[b];m&&_&&n.push({coords:[[m[0],m[1]],[_[0],_[1]]],color:"#dc2626",weight:5,zoomTo:u===0}),m&&!d.has(g)&&(d.add(g),i.push({lat:m[0],lon:m[1],label:`<code>${g}</code>`,primary:!0,small:!1})),_&&!d.has(b)&&(d.add(b),i.push({lat:_[0],lon:_[1],label:`<code>${b}</code>`,primary:!0,small:!1}))}return n.length>0&&r.push({color:"#dc2626",label:"Bozuk segment"}),r.push({color:"#2563eb",label:"Segment uçları"}),{pins:i,polyline:l.length>1?l.map(u=>[u[0],u[1]]):void 0,extraPolylines:n,legendItems:r,showArrows:!1}}if(e.rule_id==="STM_025"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",i=e.details?.stop_b??"",d=[];for(const m of r){if(m===n||m===i)continue;const _=v(m,t,!0);_&&d.push({..._,small:!0})}const u=n?v(n,t,!0,"(kalkış)"):null;u&&d.push(u);const g=i?v(i,t,!1,"(varış — çok kısa süre)"):null;g&&d.push(g);const b=[];return l.length>1&&b.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&b.push({color:"#2563eb",label:"Sefer durakları"}),b.push({color:"#dc2626",label:"Kısa süreli segment sonu"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:b,showArrows:!0}}if(e.rule_id==="OPR_015"){const a=e.details?.shape_id??"",l=a?t.shape_coords[a]??[]:[],r=l.length>1?[{color:"#f59e0b",label:"Tek güzergah şekli"}]:[];return{pins:[],polyline:l.length>1?l:void 0,legendItems:r,showArrows:!0}}function o(a){const l=t.shape_trips[a]??"";return(l?t.trip_stops[l]??[]:[]).map(n=>v(n,t,!0)).filter(n=>n!==null).map(n=>({...n,small:!0}))}if(e.rule_id==="SHP_007"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:a.length>1}}if(e.rule_id==="SHP_012"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:!0}}if(e.rule_id==="SHP_014"){const a=s?t.shape_coords[s]??[]:[],l=e.details?.problem_stop??"",r=e.details?.trip_id??"",n=r?t.trip_stops[r]??[]:[],i=[];for(const b of n){if(b===l)continue;const m=v(b,t,!0);m&&i.push({...m,small:!0})}const d=e.details?.endpoint==="start"?"⚠ ilk durak — shape başından uzak":"⚠ son durak — shape bitişinden uzak",u=l?v(l,t,!1,d):null;u&&i.push(u);const g=[];return a.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),u&&g.push({color:"#dc2626",label:"Uçtan uzak durak"}),{pins:i,polyline:a.length>1?a:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="SHP_015"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:!0}}if(e.rule_id==="SHP_016"){const a=s?t.shape_coords[s]??[]:[],l=e.details?.first_stop??"",r=e.details?.trip_id??"",n=r?t.trip_stops[r]??[]:[],i=[];for(const g of n){if(g===l)continue;const b=v(g,t,!0);b&&i.push({...b,small:!0})}const d=l?v(l,t,!1,"⚠ ilk durak — shape'in yanlış ucuna düşüyor"):null;d&&i.push(d);const u=[];return a.length>1&&u.push({color:"#f59e0b",label:"Güzergah şekli (ters)"}),n.length>0&&u.push({color:"#2563eb",label:"Sefer durakları"}),d&&u.push({color:"#dc2626",label:"İlk durak (yanlış uçta)"}),{pins:i,polyline:a.length>1?a:void 0,legendItems:u,showArrows:!0}}if(e.rule_id==="STM_012"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",i=e.details?.stop_b??"",d=[];for(const m of r){if(m===n||m===i)continue;const _=v(m,t,!0);_&&d.push({..._,small:!0})}const u=n?v(n,t,!0,"(kalkış — imkansız hız)"):null,g=i?v(i,t,!1,"(varış — imkansız hız)"):null;u&&d.push(u),g&&d.push(g);const b=[];return l.length>1&&b.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&b.push({color:"#2563eb",label:"Sefer durakları"}),b.push({color:"#dc2626",label:"İmkansız hız segmenti"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:b,showArrows:!0}}if(e.rule_id==="STM_026"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",i=e.details?.stop_b??"",d=[];for(const m of r){if(m===n||m===i)continue;const _=v(m,t,!0);_&&d.push({..._,small:!0})}const u=n?v(n,t,!0,"(kalkış)"):null,g=i?v(i,t,!1,"(varış — çok uzak)"):null;u&&d.push(u),g&&d.push(g);const b=[];return l.length>1&&b.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&b.push({color:"#2563eb",label:"Sefer durakları"}),b.push({color:"#dc2626",label:"Aşırı uzak segment sonu"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:b,showArrows:!0}}if(e.rule_id==="STM_033"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(d=>v(d,t,!1,"(tek durak — sefer kullanılamaz)")).filter(d=>d!==null),i=[];return l.length>1&&i.push({color:"#f59e0b",label:"Güzergah şekli"}),i.push({color:"#dc2626",label:"Tek durak (kullanılamaz sefer)"}),{pins:n,polyline:l.length>1?l:void 0,legendItems:i,showArrows:l.length>1}}if(e.rule_id==="PTH_014"){const a=e.details?.from_station??"",l=e.details?.to_station??"",r=t.stop_coords[a],n=t.stop_coords[l];if(!r||!n)return defaultMapOptions(e,t);const i=t.stops[a]??a,d=t.stops[l]??l;return{center:[(r[0]+n[0])/2,(r[1]+n[1])/2],zoom:15,markers:[{lat:r[0],lon:r[1],color:"#ef4444",label:i,title:`İstasyon: ${i}`},{lat:n[0],lon:n[1],color:"#3b82f6",label:d,title:`İstasyon: ${d}`}],polylines:[{coords:[[r[0],r[1]],[n[0],n[1]]],color:"#ef4444",dashArray:"6,4",weight:2}],showArrows:!1}}if(e.rule_id==="VAT_005"){const a=new Set((e.details?.isolated_stops??"").split(",").filter(Boolean)),l=new Set;for(const n of Object.values(t.trip_stops))for(const i of n)l.add(i);const r=[];for(const[n,i]of Object.entries(t.stop_coords)){const d=a.has(n),u=l.has(n);if(!d&&!u)continue;const g=t.stops[n]??n;r.push({lat:i[0],lon:i[1],label:d?`<strong>⚠ İzole</strong><br><strong>${g}</strong><br><code>${n}</code>`:`<strong>${g}</strong><br><code>${n}</code>`,primary:!d,small:!d})}return{pins:r,legendItems:[{color:"#2563eb",label:"Bağlı duraklar"},{color:"#dc2626",label:"İzole duraklar"}]}}if(e.rule_id==="VAT_007"){const a=(e.details?.routes??"").split(",").filter(Boolean),l=["#3b82f6","#10b981","#8b5cf6","#f97316","#06b6d4"],r=t.stop_coords[s],n=t.stops[s]??s,i=r?[{lat:r[0],lon:r[1],label:`<strong>Terminal: ${n}</strong><br><code>${s}</code>`,primary:!1}]:[],d=[],u=[];i.length>0&&u.push({color:"#dc2626",label:"Terminal durağı"});let g=0;for(const b of a.slice(0,5)){const m=t.route_shapes[b]??[],_=l[g%l.length];let y=!1;for(const k of m.slice(0,2)){const w=t.shape_coords[k]??[];w.length>1&&(d.push({coords:w,color:_,weight:3,zoomTo:!1}),y=!0)}if(y){const k=t.routes[b]??b;u.push({color:_,label:k}),g++}}return{pins:i,extraPolylines:d,legendItems:u,showArrows:!1}}if(e.rule_id==="GEO_013"){const a=Object.entries(t.stop_coords).map(([l,r])=>({lat:r[0],lon:r[1],label:t.stops[l]?`<strong>${t.stops[l]}</strong><br><code>${l}</code>`:`<code>${l}</code>`,primary:!0,small:!0}));return{pins:a,legendItems:[{color:"#2563eb",label:`${a.length} durak`}]}}if(Q.has(e.rule_id)){const a=t.trip_shapes[s]??"",l=a?t.shape_coords[a]??[]:[],r=t.trip_stops[s]??[],n=t.trip_routes[s]??"",i=n?t.routes[n]??"":"",d=r.flatMap((g,b)=>{const m=v(g,t,!0,`#${b+1}`);return m?[{...m,small:!0}]:[]}),u=[];return l.length>1&&u.push({color:"#f59e0b",label:i?`Güzergah şekli — ${i}`:"Güzergah şekli"}),d.length>0&&u.push({color:"#2563eb",label:"Sefer durakları"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:u,showArrows:l.length>1}}if(J.has(e.rule_id)){const a=t.route_shapes[s]??[],l=t.routes[s]??s,r=[];for(const n of a.slice(0,4)){const i=t.shape_coords[n]??[];i.length>1&&r.push({coords:i,color:"#f59e0b",weight:3,zoomTo:r.length===0})}return r.length===0?{pins:[]}:{pins:[],extraPolylines:r,legendItems:[{color:"#f59e0b",label:`Güzergah şekilleri — ${l}`}],showArrows:!1}}const c=t.stop_coords[s];if(!c)return{pins:[]};const[f,p]=c,h=t.stops[s]??s;if(e.rule_id==="STP_029"){const a=e.message.match(/üst istasyonundan \(([^)]+)\)/)?.[1]??"",l=[{lat:f,lon:p,label:`<strong>${h}</strong><br><code>${s}</code>`,primary:!0}],r=a?v(a,t,!1,"(üst istasyon)"):null;r&&l.push(r);const n=l.length>1?[{color:"#2563eb",label:"Durak"},{color:"#dc2626",label:"Üst İstasyon"}]:[];return{pins:l,legendItems:n}}if(e.rule_id==="STP_016"){const a=e.observed_value?.match(/== '([^']+)'/)?.[1]??"",l=a?t.stops[a]??a:"",r=l?`<strong>${h}</strong> = <strong>${l}</strong><br><code>${s}</code> ve <code>${a}</code>`:`<strong>${h}</strong><br><code>${s}</code>`;return{pins:[{lat:f,lon:p,label:r,primary:!0}]}}if(e.rule_id==="STP_017"){const a=e.observed_value?.match(/to '([^']+)'/)?.[1]??"",l=[{lat:f,lon:p,label:`<strong>${h}</strong><br><code>${s}</code>`,primary:!0}],r=a?v(a,t,!1):null;r&&l.push(r);const n=l.length>1?[{color:"#2563eb",label:"Durak 1"},{color:"#dc2626",label:"Durak 2"}]:[];return{pins:l,legendItems:n}}if(e.rule_id==="GEO_009"){const a=e.observed_value?.match(/\(shape '([^']+)'\)/)?.[1]??"",l=a?t.shape_coords[a]??[]:[],r=[{color:"#2563eb",label:"Durak"}];return l.length>0&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),{pins:[{lat:f,lon:p,label:`<strong>${h}</strong><br><code>${s}</code>`,primary:!0}],polyline:l.length>1?l:void 0,legendItems:r}}return{pins:[{lat:f,lon:p,label:`<strong>${h}</strong><br><code>${s}</code>`,primary:!0}]}}function Ve(e,t,s){if(e.querySelectorAll(".r9-main-row").forEach(i=>{i.addEventListener("click",()=>{const d=i.dataset.idx,u=e.querySelector(`.r9-detail-row[data-for="${d}"]`);if(!u)return;u.hidden=!u.hidden;const g=i.querySelector(".r9-arrow");g&&(g.textContent=u.hidden?"▶":"▼")})}),t){const i=new Map(t.notices.map(d=>[d.id,d]));e.querySelectorAll(".map-pin-btn").forEach(d=>{d.addEventListener("click",u=>{u.stopPropagation();const g=d.dataset.noticeId??"",b=i.get(g);if(!b)return;const m=Ye(b,t.name_index);if(m.pins.length===0&&!m.polyline)return;const _=new Set(["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"]),y=b.entity_id??"";let k;if(_.has(b.rule_id))k=`şekil: ${y}`;else if(Q.has(b.rule_id)){const w=t.name_index.trip_routes[y]??"",z=w?t.name_index.routes[w]??"":"",M=t.name_index.trips[y]??"";k=z?`${z}${M?` — ${M}`:""}`:M||y}else J.has(b.rule_id)?k=t.name_index.routes[y]??y:k=t.name_index.stops[y]??y;De(`${b.rule_id} — ${k}`,m)})})}const o=e.querySelector("#sev-filter"),c=e.querySelector("#cls-filter"),f=e.querySelector("#rule-filter"),p=e.querySelector("#r2-table");if(!o||!c||!p)return;const h=f?Array.from(f.options).filter(i=>i.value!=="").map(i=>[i.value,i.text]):[],a=[["CRITICAL","KRİTİK"],["HIGH","YÜKSEK"],["MEDIUM","ORTA"],["LOW","DÜŞÜK"],["INFO","BİLGİ"]],l=[["SPEC","GTFS Geçerliliği"],["INTEROP","GTFS Uyumluluğu"],["QUALITY","GTFS Kalitesi"],["ANALYTICS","GTFS Analitiği"]];function r(i,d,u,g){i.innerHTML='<option value="">Tümü</option>'+g.filter(([b])=>u.has(b)).map(([b,m])=>`<option value="${b}"${b===d?" selected":""}>${m}</option>`).join(""),d&&!u.has(d)&&(i.value="")}function n(){const i=o.value,d=c.value,u=f?.value??"";let g=0,b=0;const m=new Set,_=new Set,y=new Set;p.querySelectorAll("tbody tr").forEach(R=>{b++;const x=R.dataset.severity??"",ee=R.dataset.class??"",te=R.dataset.rule??"",B=!i||x===i,I=!d||ee===d,N=!u||te===u;R.style.display=B&&I&&N?"":"none",B&&I&&N&&g++,I&&N&&m.add(x),B&&N&&_.add(ee),B&&I&&y.add(te)});const k=i||d||u,w=e.querySelector("#filter-count");w&&(w.textContent=k?`${b} noticeden ${g} tanesi`:"");const z=e.querySelector("#r2-card h2 .count-badge");z&&(z.textContent=String(k?g:b));const M=e.querySelector("#r2-cap-warning");if(M){const R=u&&s?s[u]:void 0;R!=null?(M.hidden=!1,M.textContent=`⚠ Bu kural cap'e çarptı — raporda yalnızca ${g.toLocaleString("tr-TR")} notice gösterilmektedir, gerçek toplam: ${R.toLocaleString("tr-TR")}.`):M.hidden=!0}r(o,i,m,a),r(c,d,_,l),f&&(f.innerHTML='<option value="">Tümü</option>'+h.filter(([R])=>y.has(R)).map(([R,x])=>`<option value="${R}"${R===u?" selected":""}>${T(x)}</option>`).join(""),u&&!y.has(u)&&(f.value=""))}o.addEventListener("change",n),c.addEventListener("change",n),f?.addEventListener("change",n)}const F={CRITICAL:0,HIGH:1,MEDIUM:2,LOW:3,INFO:4},Ze={Stop:"Durak",Trip:"Sefer",Route:"Hat",Shape:"Güzergah",Agency:"İşletici",Service:"Servis",File:"Dosya",Feed:"Feed",Row:"Satır",Field:"Alan",Transfer:"Aktarma",Pathway:"Geçit",Level:"Kat",Translation:"Çeviri",Attribution:"Atıf",Fare:"Ücret"};function We(e,t){const s=Qe(t.notices),o=t.notices.length;e.innerHTML=`
    <div class="rules-page">
      <div class="rules-summary-bar">
        <span class="rules-summary-text">${o} bulgu · ${s.length} kural</span>
      </div>
      <div class="rules-accordion">
        ${s.map(c=>Je(c)).join("")}
      </div>
    </div>`,Xe(e)}function Qe(e){const t=new Map;for(const s of e){t.has(s.rule_id)||t.set(s.rule_id,{rule_id:s.rule_id,max_severity:s.severity,rule_class:s.rule_class,notices:[]});const o=t.get(s.rule_id);o.notices.push(s),(F[s.severity]??9)<(F[o.max_severity]??9)&&(o.max_severity=s.severity)}return Array.from(t.values()).sort((s,o)=>{const c=(F[s.max_severity]??9)-(F[o.max_severity]??9);return c!==0?c:o.notices.length-s.notices.length})}function Je(e){const t=j[e.max_severity],s=O[e.max_severity],o=U[e.rule_class]??e.rule_class,c=e.rule_class.toLowerCase(),p=[...e.notices].sort((a,l)=>(F[a.severity]??9)-(F[l.severity]??9)).map(a=>{const l=et(a.message),r=l.length>160?l.slice(0,160)+"…":l,n=j[a.severity]??"#666",i=Ze[a.entity_type]??a.entity_type,d=a.entity_id?`<span class="muted-text">${A(i)}</span> <code class="entity-id">${A(a.entity_id)}</code>`:`<span class="muted-text">${A(i)}</span>`;return`
      <tr>
        <td style="color:${n};white-space:nowrap">${O[a.severity]}</td>
        <td>${d}</td>
        <td class="msg-cell">${A(r)}</td>
      </tr>`}).join(""),h=e.notices[0]?.remediation??"";return`
    <div class="rule-acc-section">
      <button class="rule-acc-header" type="button" aria-expanded="false">
        <span class="acc-arrow">▶</span>
        <span class="rule-title-block">
          <code class="rule-code rule-code-lg">${A(e.rule_id)}</code>
          ${h?`<span class="rule-desc-text">${A(h)}</span>`:""}
        </span>
        <span class="badge badge-${c}">${A(o)}</span>
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
    </div>`}function Xe(e){e.querySelectorAll(".rule-acc-header").forEach(t=>{t.addEventListener("click",()=>{const o=t.closest(".rule-acc-section").querySelector(".rule-acc-body"),c=t.querySelector(".acc-arrow"),f=!o.hidden;o.hidden=f,c.textContent=f?"▶":"▼",t.setAttribute("aria-expanded",String(!f))})})}function et(e){return e.replace(/\btrip_id\b/g,"sefer").replace(/\bstop_sequence\b/g,"Durak Sırası").replace(/\barrival_time\b/g,"Varış zamanı").replace(/\bdeparture_time\b/g,"Kalkış zamanı").replace(/\bheadway\b/g,"Sefer Aralığı")}function A(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function tt(e,t,s){e.innerHTML=`
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
    </section>`,e.querySelector("#btn-export-html").addEventListener("click",()=>at(t,s)),e.querySelector("#btn-export-csv").addEventListener("click",()=>st(t,s)),e.querySelector("#btn-export-json").addEventListener("click",()=>lt(t,s)),e.querySelector("#btn-export-pdf").addEventListener("click",()=>ot(t,s))}function re(e){const t=e==null?"":String(e);return t.includes(",")||t.includes('"')||t.includes(`
`)?`"${t.replace(/"/g,'""')}"`:t}function st(e,t){const s=["Kural","Önem","Sınıf","Mesaj","Varlık ID","Dosya","Satır"],o=e.notices.map(h=>[h.rule_id,O[h.severity]??h.severity,U[h.rule_class]??h.rule_class,h.message,h.entity_id??"",h.file??"",h.line!=null?String(h.line):""].map(re).join(",")),f="\uFEFF"+[s.map(re).join(","),...o].join(`\r
`),p=new Blob([f],{type:"text/csv; charset=utf-8"});X(p,t.replace(/\.zip$/i,"-report.csv"))}function lt(e,t){const s=new Blob([JSON.stringify(e,null,2)],{type:"application/json"});X(s,t.replace(/\.zip$/i,"-report.json"))}function be(e,t){const{r1:s,r5:o}=e.reports,c=s.publishable?s.conditional?"Koşullu Yayına Uygun":"Yayına Uygun":"Yayınlanması Tavsiye Edilmez",f=e.notices.map(p=>`
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
      <span class="kpi-value">${c}</span>
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
    <tbody>${f}</tbody>
  </table>
</body>
</html>`}function at(e,t){const s=be(e,t),o=new Blob([s],{type:"text/html; charset=utf-8"});X(o,t.replace(/\.zip$/i,"-report.html"))}function ot(e,t){const s=be(e,t),o=window.open("","_blank");if(!o){alert("Açılır pencere engellendi. Tarayıcı ayarlarını kontrol edin.");return}o.document.write(s),o.document.close(),o.focus(),o.print()}function X(e,t){const s=URL.createObjectURL(e),o=document.createElement("a");o.href=s,o.download=t,o.click(),URL.revokeObjectURL(s)}function E(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}const ne={domain:"Rapor",fix:"Ayrıntı ve Düzeltme",rules:"Kategori Bazlı",export:"Dışa Aktar"};function rt(){localStorage.getItem("gtfs-theme")==="dark"&&document.documentElement.classList.add("dark")}function ie(){const e=document.documentElement.classList.toggle("dark");localStorage.setItem("gtfs-theme",e?"dark":"light"),nt()}function nt(){const e=document.documentElement.classList.contains("dark");document.querySelectorAll(".dark-toggle").forEach(t=>{t.textContent=e?"☀":"☾",t.title=e?"Açık mod":"Koyu mod"})}function ce(){const e=document.documentElement.classList.contains("dark");return`<button class="btn btn-ghost dark-toggle" title="${e?"Açık mod":"Koyu mod"}">${e?"☀":"☾"}</button>`}function H(){const e=Z(),t=document.getElementById("app");if(e.page==="upload"||!e.result){t.innerHTML=`
      <header class="app-header">
        <h1>GTFS Analyzer</h1>
        <span style="flex:1"></span>
        ${ce()}
      </header>
      <main id="page-root"></main>`,t.querySelector(".dark-toggle").addEventListener("click",ie),Se(document.getElementById("page-root"));return}const s=Object.keys(ne).map(c=>`
      <button class="nav-btn ${e.page===c?"active":""}" data-page="${c}">
        ${ne[c]}
      </button>`).join("");t.innerHTML=`
    <header class="app-header">
      <h1>GTFS Validator</h1>
      <span class="header-filename">${it(e.fileName)}</span>
      <button id="btn-back" class="btn btn-ghost">← Ana Sayfa</button>
      <button id="btn-new" class="btn btn-secondary">Yeni GTFS Yükle</button>
      ${ce()}
    </header>
    <nav class="app-nav">${s}</nav>
    <main id="page-root"></main>`,t.querySelector("#btn-back").addEventListener("click",()=>{D("upload"),H()}),t.querySelector("#btn-new").addEventListener("click",()=>{D("upload"),H()}),t.querySelector(".dark-toggle").addEventListener("click",ie),t.querySelectorAll(".nav-btn").forEach(c=>{c.addEventListener("click",()=>{D(c.dataset.page),H()})});const o=document.getElementById("page-root");switch(e.page){case"domain":Me(o,e.result);break;case"fix":Be(o,e.result),Ve(o,e.result,e.result.capped_totals);break;case"rules":We(o,e.result);break;case"export":tt(o,e.result,e.fileName);break}}function it(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}rt();H();
