(function(){const t=document.createElement("link").relList;if(t&&t.supports&&t.supports("modulepreload"))return;for(const i of document.querySelectorAll('link[rel="modulepreload"]'))o(i);new MutationObserver(i=>{for(const f of i)if(f.type==="childList")for(const p of f.addedNodes)p.tagName==="LINK"&&p.rel==="modulepreload"&&o(p)}).observe(document,{childList:!0,subtree:!0});function s(i){const f={};return i.integrity&&(f.integrity=i.integrity),i.referrerPolicy&&(f.referrerPolicy=i.referrerPolicy),i.crossOrigin==="use-credentials"?f.credentials="include":i.crossOrigin==="anonymous"?f.credentials="omit":f.credentials="same-origin",f}function o(i){if(i.ep)return;i.ep=!0;const f=s(i);fetch(i.href,f)}})();const A={page:"upload",result:null,configDelta:sessionStorage.getItem("gtfs-config-delta")??"",fileName:""};function N(){return A}function ie(e,t){A.result=e,A.fileName=t,A.page="domain"}function q(e){A.page=e}function te(e){A.configDelta=e,sessionStorage.setItem("gtfs-config-delta",e)}const F={CRITICAL:"KRİTİK",HIGH:"YÜKSEK",MEDIUM:"ORTA",LOW:"DÜŞÜK",INFO:"BİLGİ"},B={SPEC:"GTFS Geçerliliği",INTEROP:"GTFS Uyumluluğu",QUALITY:"GTFS Kalitesi",ANALYTICS:"GTFS Analitiği"},ce={ZipUnreadable:"ZIP dosyası açılamadı",Utf8Critical:"UTF-8 kodlama hatası",NoRequiredFiles:"Zorunlu dosyalar eksik",CsvMalformed:"CSV biçim hatası",ResourceLimit:"Notice limiti aşıldı",InvalidInput:"Geçersiz girdi"},K={CRITICAL:"var(--color-critical)",HIGH:"var(--color-high)",MEDIUM:"var(--color-medium)",LOW:"var(--color-low)",INFO:"var(--color-info)"},I=new Worker(new URL(""+new URL("validator-worker-LZfMk71Y.js",import.meta.url).href,import.meta.url),{type:"module"}),D=new Map;let de=1;I.onmessage=e=>{const t=e.data,s=D.get(t.id);if(s){if(t.type==="file-list"){s.callbacks?.onFileList?.(t.files);return}if(t.type==="file-done"){s.callbacks?.onFileDone?.(t.name,t.rows,t.bytes);return}if(t.type==="stage"){s.callbacks?.onStageDone?.(t.stage,t.elapsed_ms);return}t.type==="result"&&(D.delete(t.id),t.ok?s.resolve(t.result):s.reject(t.error))}};I.onerror=e=>{const t={code:"ResourceLimit",message:e.message||"Arka plan doğrulama işçisi beklenmedik şekilde durdu."};for(const[,s]of D)s.reject(t);D.clear()};function pe(e,t,s){const o=de++;return new Promise((i,f)=>{D.set(o,{resolve:i,reject:f,callbacks:s}),I.postMessage({id:o,type:"validate",buffer:e,configDelta:t},[e])})}const se={K1:"K1 — Dosya Ayrıştırma",K2:"K2 — Kural Doğrulama",K3:"K3 — Varlık Haritası",K4:"K4 — Çapraz Kontrol",K5:"K5 — Türetilmiş Veri",K6:"K6 — İstatistik Analizi",K7:"K7 — Rapor Oluşturma"},O=["K1","K2","K3","K4","K5","K6","K7"],ue=[{key:"max_speed_bus_kmh",label:"Maks. Otobüs Hızı",unit:"km/s",type:"float",def:120,min:60,max:200,desc:"Otobüs seferleri için maksimum izin verilen hız"},{key:"max_speed_tram_kmh",label:"Maks. Tramvay Hızı",unit:"km/s",type:"float",def:100,min:40,max:160,desc:"Tramvay seferleri için maksimum izin verilen hız"},{key:"max_speed_metro_kmh",label:"Maks. Metro Hızı",unit:"km/s",type:"float",def:150,min:80,max:250,desc:"Metro seferleri için maksimum izin verilen hız"},{key:"max_speed_rail_kmh",label:"Maks. Demiryolu Hızı",unit:"km/s",type:"float",def:300,min:100,max:400,desc:"Demiryolu seferleri için maksimum izin verilen hız"},{key:"max_speed_ferry_kmh",label:"Maks. Feribot Hızı",unit:"km/s",type:"float",def:80,min:20,max:150,desc:"Feribot seferleri için maksimum izin verilen hız"},{key:"max_speed_cablecar_kmh",label:"Maks. Teleferik Hızı",unit:"km/s",type:"float",def:30,min:10,max:60,desc:"Teleferik/füniküler için maksimum izin verilen hız"},{key:"min_transfer_time_sec",label:"Min. Aktarma Süresi",unit:"sn",type:"int",def:180,min:30,max:1800,desc:"Transferler için minimum bağlantı süresi"},{key:"max_transfer_distance_m",label:"Maks. Aktarma Mesafesi",unit:"m",type:"float",def:500,min:50,max:2e3,desc:"Transfer geçerli sayılmak için maksimum mesafe"},{key:"max_shape_jump_km",label:"Maks. Güzergah Sıçraması",unit:"km",type:"float",def:10,min:1,max:50,desc:"Art arda güzergah noktaları arasındaki maksimum mesafe"},{key:"stop_too_close_m",label:"Çok Yakın Durak Eşiği",unit:"m",type:"float",def:5,min:1,max:20,desc:"Bu mesafeden yakın duraklar tekrar sayılır"},{key:"stop_far_from_shape_m",label:"Durağın Güzergah'a Uzaklığı",unit:"m",type:"float",def:100,min:20,max:500,desc:"Durağın güzergahından en fazla bu kadar uzakta olabilir"},{key:"stop_far_from_parent_m",label:"Üst İstasyona Uzaklık Eşiği",unit:"m",type:"float",def:100,min:10,max:1e3,desc:"Durak, üst istasyonundan en fazla bu kadar uzakta olabilir"},{key:"feed_expiry_warning_days",label:"Son Kullanma Uyarısı",unit:"gün",type:"int",def:30,min:1,max:60,desc:"Feed bu kadar günden az kalmışsa uyarı üretilir"},{key:"service_gap_days",label:"Servis Boşluğu Eşiği",unit:"gün",type:"int",def:7,min:3,max:30,desc:"Bu günden uzun servis kesintisi işaretlenir"},{key:"max_trip_duration_hours",label:"Maks. Sefer Süresi",unit:"saat",type:"float",def:24,min:8,max:72,desc:"Tek bir seferin maksimum süresi"},{key:"min_trip_duration_sec",label:"Min. Sefer Süresi",unit:"sn",type:"int",def:60,min:10,max:300,desc:"Tek bir seferin minimum süresi"},{key:"max_headway_warning_min",label:"Maks. Sefer Aralığı",unit:"dk",type:"int",def:240,min:60,max:720,desc:"Bu dakikadan uzun aralık uyarı üretir"},{key:"bunching_threshold_min",label:"Sıkışma Eşiği",unit:"dk",type:"int",def:2,min:1,max:10,desc:"Bu dakikadan kısa aralık sıkışma sayılır"}],le={"stops.txt":"durak","routes.txt":"hat","trips.txt":"sefer","stop_times.txt":"geçiş zamanı","shapes.txt":"güzergah","agency.txt":"işletici","calendar.txt":"takvim","calendar_dates.txt":"özel gün","frequencies.txt":"frekans","transfers.txt":"aktarma","pathways.txt":"geçit","levels.txt":"kat","attributions.txt":"atıf","translations.txt":"çeviri"};function fe(e){const t=N(),s=t.result!==null,o=s?"Farklı GTFS Yükle":"GTFS Yükle";e.innerHTML=`
    <div class="up-grid">

      <!-- Sol üst: GTFS Dosyaları -->
      <div class="up-cell up-files">
        <div class="lp-section-title">GTFS Dosyaları</div>
        <div id="file-rows" class="lp-rows">
          ${s&&t.result.metrics.file_stats.length>0?ae(t.result.metrics.file_stats):'<div class="lp-placeholder">Henüz dosya yüklenmedi</div>'}
        </div>
      </div>

      <!-- Sağ üst: ZIP Yükle -->
      <div class="up-cell up-drop">
        <div id="drop-zone" class="drop-zone" role="button" tabindex="0" aria-label="ZIP dosyası yükle">
          <svg class="upload-icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5"/>
          </svg>
          <span class="drop-primary">ZIP sürükleyin</span>
          <label id="upload-btn-label" class="btn btn-primary btn-sm" for="file-input">${w(o)}</label>
          <input type="file" id="file-input" accept=".zip" class="visually-hidden" aria-label="ZIP dosyası seç"/>
        </div>
        <div id="upload-error" class="upload-status hidden"></div>
      </div>

      <!-- Sol alt: İşlem Aşamaları -->
      <div class="up-cell up-stages">
        <div class="lp-section-title">İşlem Aşamaları</div>
        <div id="stage-rows" class="lp-rows">
          ${O.map(i=>`
            <div class="lp-stage-row ${s?"done":"waiting"}" data-stage="${i}">
              <span class="lp-icon">${s?"✓":"○"}</span>
              <span class="lp-label">${w(se[i]??i)}</span>
              <span class="lp-status">${s?"":"—"}</span>
            </div>`).join("")}
        </div>
      </div>

      <!-- Sağ alt: Kalite Skoru + Rapor butonu -->
      <div class="up-cell up-score">
        <div id="score-panel" class="score-compact${s?"":" hidden"}">
          ${s?oe(t.result):""}
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
          ${he(be(t.configDelta))}
        </div>
        <div class="settings-actions">
          <button id="btn-apply-settings" class="btn btn-primary btn-sm">Uygula</button>
          <button id="btn-reset-settings" class="btn btn-ghost btn-sm">Sıfırla</button>
          <span id="settings-status" class="settings-status hidden"></span>
        </div>
      </div>
    </div>`,ge(e)}function ae(e){return e.map(t=>{const s=Math.round(t.bytes/1024),o=le[t.name]??"satır";return`
      <div class="lp-file-row done">
        <span class="lp-icon">✓</span>
        <span class="lp-label">${w(t.name)}</span>
        <span class="lp-status">${t.rows.toLocaleString("tr-TR")} ${o} · ${x(s)}</span>
      </div>`}).join("")}function oe(e){const{r5:t}=e.reports,s=[{label:"GTFS Geçerliliği",weight:"×40%",value:t.spec_score,color:"#2563eb",bg:"#eff6ff"},{label:"GTFS Uyumluluğu",weight:"×30%",value:t.interop_score,color:"#92400e",bg:"#fef3c7"},{label:"GTFS Kalitesi",weight:"×20%",value:t.quality_score,color:"#15803d",bg:"#f0fdf4"},{label:"GTFS Analitiği",weight:"×10%",value:t.analytics_score,color:"#6d28d9",bg:"#f5f3ff"}],o=t.score>=80?"#15803d":t.score>=60?"#92400e":"#dc2626",i=t.pub_score>=80?"#15803d":t.pub_score>=60?"#d97706":"#dc2626",f=s.map(p=>`<div class="sc-chip" style="background:${p.bg};color:${p.color}">
      <div class="sc-chip-left">
        <span class="sc-chip-label">${p.label}</span>
        <span class="sc-weight">${p.weight}</span>
      </div>
      <span class="sc-chip-val">${p.value.toFixed(1)}</span>
    </div>`).join("");return`
    <div class="sc-dual-header">
      <div class="sc-dual-item">
        <span class="sc-title">Yayın Skoru</span>
        <span class="sc-overall" style="color:${i}">${t.pub_score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
      <div class="sc-dual-sep"></div>
      <div class="sc-dual-item">
        <span class="sc-title">Kalite Skoru</span>
        <span class="sc-overall" style="color:${o}">${t.score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
    </div>
    <div class="sc-chips">${f}</div>`}function he(e){return ue.map(t=>{const s=e[t.key]!==void 0?e[t.key]:t.def,o=e[t.key]!==void 0;return`
      <div class="sf-row${o?" sf-overridden":""}">
        <label class="sf-label" title="${w(t.desc)}">${w(t.label)}</label>
        <div class="sf-control">
          <input class="sf-input" type="number" data-key="${t.key}" data-def="${t.def}"
            data-type="${t.type}" min="${t.min}" max="${t.max}"
            step="${t.type==="int"?1:.1}" value="${s}"/>
          <span class="sf-unit">${t.unit}</span>
          <button class="sf-reset btn btn-ghost" data-key="${t.key}" title="Sıfırla"
            ${o?"":'style="visibility:hidden"'}>✕</button>
        </div>
        <div class="sf-desc">${w(t.desc)} (varsayılan: ${t.def})</div>
      </div>`}).join("")}function be(e){if(!e||e==="{}")return{};try{return JSON.parse(e)}catch{return{}}}function ge(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),o=e.querySelector("#upload-error");s.addEventListener("change",()=>{const u=s.files?.[0];u&&V(u,e,o)}),t.addEventListener("dragover",u=>{u.preventDefault(),t.classList.contains("loading")||t.classList.add("drag-over")}),t.addEventListener("dragleave",()=>t.classList.remove("drag-over")),t.addEventListener("drop",u=>{if(u.preventDefault(),t.classList.remove("drag-over"),t.classList.contains("loading"))return;const a=u.dataTransfer?.files[0];a&&V(a,e,o)}),t.addEventListener("keydown",u=>{(u.key==="Enter"||u.key===" ")&&!t.classList.contains("loading")&&s.click()}),e.querySelector("#btn-report")?.addEventListener("click",()=>{q("domain"),E()});const i=e.querySelector("#settings-toggle"),f=e.querySelector("#settings-body"),p=e.querySelector("#settings-arrow");i.addEventListener("click",()=>{const u=!f.classList.contains("hidden");f.classList.toggle("hidden",u),p.textContent=u?"▶":"▼"}),e.querySelector("#btn-apply-settings")?.addEventListener("click",()=>me(e)),e.querySelector("#btn-reset-settings")?.addEventListener("click",()=>_e(e)),e.querySelectorAll(".sf-reset").forEach(u=>{u.addEventListener("click",()=>{const a=u.dataset.key,l=e.querySelector(`.sf-input[data-key="${CSS.escape(a)}"]`);l&&(l.value=l.dataset.def,l.closest(".sf-row")?.classList.remove("sf-overridden"),u.style.visibility="hidden")})}),e.querySelectorAll(".sf-input").forEach(u=>{u.addEventListener("change",()=>{const a=e.querySelector(`.sf-reset[data-key="${CSS.escape(u.dataset.key)}"]`);a&&(a.style.visibility="visible"),u.closest(".sf-row")?.classList.add("sf-overridden")})})}function me(e){const t={};e.querySelectorAll(".sf-input").forEach(i=>{const f=i.dataset.key,p=parseFloat(i.dataset.def),u=i.dataset.type==="int"?parseInt(i.value,10):parseFloat(i.value);!isNaN(u)&&u!==p&&(t[f]=u)});const s=Object.keys(t).length>0?JSON.stringify(t):"";te(s);const o=e.querySelector("#settings-status");o.textContent="Ayarlar kaydedildi. Yeni ZIP yükleyince uygulanır.",o.className="settings-status ok",setTimeout(()=>{o.className="settings-status hidden"},3e3)}function _e(e){e.querySelectorAll(".sf-input").forEach(s=>{s.value=s.dataset.def,s.closest(".sf-row")?.classList.remove("sf-overridden");const o=e.querySelector(`.sf-reset[data-key="${CSS.escape(s.dataset.key)}"]`);o&&(o.style.visibility="hidden")}),te("");const t=e.querySelector("#settings-status");t.textContent="Varsayılanlara döndürüldü.",t.className="settings-status ok",setTimeout(()=>{t.className="settings-status hidden"},3e3)}async function V(e,t,s){if(!e.name.endsWith(".zip")){Z(s,{code:"InvalidInput",message:"Yalnızca .zip uzantılı dosyalar kabul edilir."});return}s.className="upload-status hidden",ye(t,e.name,e.size);try{const o=await e.arrayBuffer(),i=await pe(o,N().configDelta,{onFileList:f=>{const p=t.querySelector("#file-rows");p&&(p.innerHTML="",f.forEach(u=>{const a=document.createElement("div");a.className="lp-file-row reading",a.dataset.file=u.name;const l=Math.round(u.uncompressed_size/1024);a.innerHTML=`
            <span class="lp-icon">○</span>
            <span class="lp-label">${w(u.name)}</span>
            <span class="lp-status">${x(l)} okunuyor…</span>`,p.appendChild(a)}))},onFileDone:(f,p,u)=>{const l=t.querySelector("#file-rows")?.querySelector(`[data-file="${CSS.escape(f)}"]`);if(!l)return;l.className="lp-file-row done",l.querySelector(".lp-icon").textContent="✓";const r=le[f]??"satır";l.querySelector(".lp-status").textContent=`${p.toLocaleString("tr-TR")} ${r} · ${x(Math.round(u/1024))}`},onStageDone:(f,p)=>{const u=t.querySelector(`[data-stage="${CSS.escape(f)}"]`);if(!u)return;u.className="lp-stage-row done",u.querySelector(".lp-icon").textContent="✓",u.querySelector(".lp-status").textContent=p<1e3?`${p} ms`:`${(p/1e3).toFixed(1)} sn`;const a=O.indexOf(f);if(a>=0&&a<O.length-1){const l=t.querySelector(`[data-stage="${CSS.escape(O[a+1])}"]`);l?.classList.contains("waiting")&&(l.className="lp-stage-row running",l.querySelector(".lp-icon").textContent="⋯",l.querySelector(".lp-status").textContent="çalışıyor…")}}});ie(i,e.name),ve(t,i)}catch(o){re(t),Z(s,o)}}function ye(e,t,s){const o=e.querySelector("#drop-zone"),i=e.querySelector("#file-input"),f=e.querySelector("#upload-btn-label"),p=e.querySelector("#score-panel");p&&(p.classList.add("hidden"),p.innerHTML="");const u=e.querySelector("#btn-report");u&&(u.disabled=!0),o.classList.add("loading"),i.disabled=!0,f.setAttribute("aria-disabled","true"),f.style.opacity="0.5",f.style.pointerEvents="none";const a=(s/1048576).toFixed(1),l=o.querySelector(".drop-primary");l&&(l.textContent=`${t} (${a} MB) işleniyor…`);const r=e.querySelector("#stage-rows");r&&(r.innerHTML=O.map((n,c)=>`
      <div class="lp-stage-row ${c===0?"running":"waiting"}" data-stage="${n}">
        <span class="lp-icon">${c===0?"⋯":"○"}</span>
        <span class="lp-label">${w(se[n]??n)}</span>
        <span class="lp-status">${c===0?"çalışıyor…":"bekliyor"}</span>
      </div>`).join(""))}function re(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),o=e.querySelector("#upload-btn-label");t.classList.remove("loading"),s.disabled=!1,o.removeAttribute("aria-disabled"),o.style.opacity="",o.style.pointerEvents=""}function ve(e,t){re(e);const s=e.querySelector("#upload-btn-label");s.textContent="Farklı GTFS Yükle";const o=e.querySelector("#score-panel");o.className="score-compact",o.innerHTML=oe(t);const i=e.querySelector("#btn-report");i.disabled=!1,i.addEventListener("click",()=>{q("domain"),E()},{once:!0});const f=e.querySelector("#file-rows");f&&t.metrics.file_stats.length>0&&(f.innerHTML=ae(t.metrics.file_stats))}function Z(e,t){const s=ce[t.code]??t.code;e.className="upload-status error",e.innerHTML=`<strong>${w(s)}</strong><p>${w(t.message)}</p>`}function x(e){return e>=1024?`${(e/1024).toFixed(1)} MB`:`${e} KB`}function w(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function ke(e,t){const{r1:s,r5:o}=t.reports,i=t.metrics,f=we(t);e.innerHTML=`
    <div class="report-page">
      ${Se(o)}
      ${$e(s,t)}
      ${Te(o)}
      ${Le(f)}
      ${ze(i)}
    </div>`}function Se(e){const t=Q(e.pub_score),s=Q(e.score);return`
    <div class="rpt-score-row">
      <div class="rpt-score-card">
        <span class="rpt-score-label">Yayın Skoru</span>
        <span class="rpt-score-val" style="color:${t}">${e.pub_score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${e.pub_score.toFixed(1)}%;background:${t}"></div></div>
        <span class="rpt-score-hint">SPEC + INTEROP blocker temizliği</span>
      </div>
      <div class="rpt-score-card">
        <span class="rpt-score-label">Kalite Skoru</span>
        <span class="rpt-score-val" style="color:${s}">${e.score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${e.score.toFixed(1)}%;background:${s}"></div></div>
        <span class="rpt-score-hint">Tüm sınıfların ağırlıklı ortalaması</span>
      </div>
    </div>`}function Q(e){return e>=80?"#16a34a":e>=60?"#d97706":"#dc2626"}function $e(e,t){const s=new Map(t.notices.map(o=>[o.id,o]));if(!e.publishable){const o=e.blocker_notice_ids.slice(0,5).map(i=>{const f=s.get(i);return f?`<li><code>${G(f.rule_id)}</code> — ${G(f.title||f.message.slice(0,80))}</li>`:""}).join("");return`
      <div class="rpt-r1 rpt-r1-blocked">
        <div class="rpt-r1-icon">✕</div>
        <div class="rpt-r1-body">
          <strong>Yayınlanması Tavsiye Edilmez</strong>
          <span>${e.blocker_notice_ids.length} kritik engelleyici bulgu mevcut — feed yayına alınamaz.</span>
          ${o?`<ul class="rpt-r1-list">${o}</ul>`:""}
        </div>
      </div>`}if(e.conditional){const o=e.conditional_blocker_notice_ids.slice(0,5).map(i=>{const f=s.get(i);return f?`<li><code>${G(f.rule_id)}</code> — ${G(f.title||f.message.slice(0,80))}</li>`:""}).join("");return`
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
    </div>`}function Te(e){return`
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
    </div>`}function we(e){const t={CRITICAL:0,HIGH:0,MEDIUM:0,LOW:0,INFO:0};for(const s of e.notices)s.severity in t&&t[s.severity]++;return t}function Le(e){const t=e.CRITICAL+e.HIGH+e.MEDIUM+e.LOW+e.INFO;if(t===0)return"";const s=[{key:"CRITICAL",label:"Kritik",color:"#dc2626",bg:"#fef2f2"},{key:"HIGH",label:"Yüksek",color:"#d97706",bg:"#fffbeb"},{key:"MEDIUM",label:"Orta",color:"#2563eb",bg:"#eff6ff"},{key:"LOW",label:"Düşük",color:"#16a34a",bg:"#f0fdf4"},{key:"INFO",label:"Bilgi",color:"#6b7280",bg:"#f9fafb"}].filter(o=>e[o.key]>0).map(o=>`
    <div class="rpt-sev-chip" style="background:${o.bg};color:${o.color}">
      <span class="rpt-sev-label">${o.label}</span>
      <span class="rpt-sev-val">${e[o.key].toLocaleString("tr-TR")}</span>
    </div>`).join("");return`
    <div class="card rpt-sev-card">
      <h3 class="rpt-section-title">Hata Dağılımı <span class="count-badge">${t.toLocaleString("tr-TR")}</span></h3>
      <div class="rpt-sev-row">${s}</div>
    </div>`}function ze(e){const s=[{label:"Durak",value:e.stop_count.toLocaleString("tr-TR")},{label:"Hat",value:e.route_count.toLocaleString("tr-TR")},{label:"Sefer",value:e.trip_count.toLocaleString("tr-TR")},{label:"Güzergah",value:e.shape_count.toLocaleString("tr-TR")},{label:"Aktif Servis Günü",value:e.active_service_days.toLocaleString("tr-TR")},{label:"Ort. Günlük Sefer",value:e.avg_daily_trips.toFixed(0)}].map(i=>`
    <div class="rpt-metric">
      <span class="rpt-metric-val">${i.value}</span>
      <span class="rpt-metric-label">${i.label}</span>
    </div>`).join(""),o=e.file_stats.map(i=>`
    <tr>
      <td><code>${G(i.name)}</code></td>
      <td class="num">${i.rows.toLocaleString("tr-TR")}</td>
      <td class="num">${Pe(i.bytes)}</td>
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
    </div>`}function Pe(e){return e<1024?`${e} B`:e<1024*1024?`${(e/1024).toFixed(1)} KB`:`${(e/1024/1024).toFixed(1)} MB`}function G(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}let S=null,k=null;function Me(e,t){Ae(),S.querySelector(".mm-title").textContent=e,S.classList.remove("mm-hidden");const s=S.querySelector(".mm-map");k?k.eachLayer(p=>{p instanceof L.TileLayer||k.removeLayer(p)}):(k=L.map(s),L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",{attribution:"© OpenStreetMap katkıcıları",maxZoom:19}).addTo(k));const o=[];if(t.polyline&&t.polyline.length>1){L.polyline(t.polyline,{color:"#f59e0b",weight:3,opacity:.8}).addTo(k);for(const p of t.polyline)o.push(p);if(t.showArrows)for(const p of Re(t.polyline,250))L.marker([p.lat,p.lon],{icon:L.divIcon({html:`<svg width="14" height="14" viewBox="-7 -7 14 14" style="transform:rotate(${p.angle}deg);display:block">
              <polygon points="0,-6 5,4 -5,4" fill="#f59e0b" stroke="#fff" stroke-width="1.5"/>
            </svg>`,className:"",iconSize:[14,14],iconAnchor:[7,7]}),interactive:!1}).addTo(k)}let i=null;if(t.extraPolylines){for(const p of t.extraPolylines)if(p.coords.length>1){L.polyline(p.coords,{color:p.color,weight:p.weight??4,opacity:.95}).addTo(k);for(const u of p.coords)o.push(u);p.zoomTo&&(i=p.coords)}}for(const p of t.pins){const u=p.primary?"#2563eb":"#dc2626",a=p.small?5:9,l=p.small?1.5:2.5;L.circleMarker([p.lat,p.lon],{radius:a,fillColor:u,color:"#fff",weight:l,fillOpacity:.85}).addTo(k).bindPopup(p.label,{maxWidth:260}),o.push([p.lat,p.lon])}i&&i.length>1?k.fitBounds(i,{padding:[60,60],maxZoom:16}):o.length===0?k.setView([39,35],6):o.length===1?k.setView(o[0],16):k.fitBounds(o,{padding:[40,40]});const f=S.querySelector(".mm-legend");t.legendItems&&t.legendItems.length>0?(f.innerHTML=t.legendItems.map(p=>`<div class="mm-legend-item"><div class="mm-dot" style="background:${p.color}"></div>${p.label}</div>`).join(""),f.style.display="flex"):f.style.display="none",setTimeout(()=>k.invalidateSize(),50)}function C(){S?.classList.add("mm-hidden")}function Re(e,t){const o=l=>l*Math.PI/180,i=(l,r,n,c)=>{const d=o(n-l),h=o(c-r),b=Math.sin(d/2)**2+Math.cos(o(l))*Math.cos(o(n))*Math.sin(h/2)**2;return 12742e3*Math.atan2(Math.sqrt(b),Math.sqrt(1-b))},f=(l,r,n,c)=>{const d=o(c-r),h=Math.sin(d)*Math.cos(o(n)),b=Math.cos(o(l))*Math.sin(o(n))-Math.sin(o(l))*Math.cos(o(n))*Math.cos(d);return(Math.atan2(h,b)*180/Math.PI+360)%360},p=[];let u=0,a=t*.5;for(let l=0;l<e.length-1;l++){const[r,n]=e[l],[c,d]=e[l+1],h=i(r,n,c,d),b=f(r,n,c,d);for(;u+h>=a;){const g=(a-u)/h;p.push({lat:r+g*(c-r),lon:n+g*(d-n),angle:b}),a+=t}u+=h}return p}function Ae(){S||(S=document.createElement("div"),S.className="mm-overlay mm-hidden",S.innerHTML=`
    <div class="mm-box" role="dialog" aria-modal="true" aria-label="Harita">
      <div class="mm-header">
        <span class="mm-title"></span>
        <button class="mm-close" title="Kapat" aria-label="Kapat">✕</button>
      </div>
      <div class="mm-map"></div>
      <div class="mm-legend"></div>
    </div>`,document.body.appendChild(S),S.querySelector(".mm-close").addEventListener("click",C),S.addEventListener("click",e=>{e.target===S&&C()}),document.addEventListener("keydown",e=>{e.key==="Escape"&&C()}))}const Ee={blocker:"Engelleyici",conditional_blocker:"Koşullu Engelleyici",interop:"Uyumluluk",propagation:"Zincirleme","quick-win":"Kolay Düzeltme",quality:"Kalite Öncelikli",widespread:"Yaygın"};function Fe(e,t){const s=new Map(t.notices.map(a=>[a.id,a])),o=t.reports.r9.items.reduce((a,l)=>a+l.score_delta,0),i=o>0?(100-t.reports.r5.score)/o:1,f=t.reports.r9.items.reduce((a,l)=>a+l.pub_score_delta,0),p=f>0?(100-t.reports.r5.pub_score)/f:1,u=new Map(t.reports.r9.items.map(a=>[a.rule_id,{qualDelta:a.score_delta*i,pubDelta:a.pub_score_delta*p,count:a.affected_instance_count}]));e.innerHTML=`
    <section class="page-fix">
      ${He(t.reports.r9.items,s,i,p)}
      ${Ge(t,s,u,t.name_index)}
    </section>`}function He(e,t,s,o){if(e.length===0)return'<div class="card"><h2>Düzeltme Kuyruğu (R9)</h2><p class="empty">Düzeltilecek bulgu yok.</p></div>';const i=e.map((f,p)=>{const u=f.notice_ids[0]?t.get(f.notice_ids[0]):void 0,a=u?.severity??"INFO",l=f.labels.map(g=>`<span class="label-badge label-${g}">${Ee[g]??g}</span>`).join(""),r=f.pub_score_delta*o,n=f.score_delta*s,c=r>=.05?`<span class="pub-delta">+${r.toFixed(1)}</span>`:'<span class="muted-text">—</span>',d=n>=.05?`<span class="score-delta">+${n.toFixed(1)}</span>`:'<span class="muted-text">—</span>',h=`
      <tr class="r9-main-row" data-idx="${p}">
        <td>
          <span class="r9-arrow">▶</span> <code>${$(f.rule_id)}</code>
          ${u?.title?`<span class="r9-rule-title">${$(u.title)}</span>`:""}
        </td>
        <td style="color:${K[a]}">${F[a]}</td>
        <td>${l}</td>
        <td>${f.affected_instance_count}</td>
        <td class="score">${f.priority_score.toFixed(1)}</td>
        <td class="score-delta-cell">${c}</td>
        <td class="score-delta-cell">${d}</td>
        <td>${f.realized_dependent_count}</td>
        <td>${f.fix_effort.toFixed(1)}</td>
      </tr>`,b=`
      <tr class="r9-detail-row" data-for="${p}" hidden>
        <td colspan="9">
          <div class="r9-detail">
            ${u?.title?`<p><strong>Mesaj:</strong> ${$(u.title)}</p>`:""}
            ${u?.remediation?`<p><strong>Çözüm:</strong> ${$(u.remediation)}</p>`:""}
          </div>
        </td>
      </tr>`;return h+b}).join("");return`
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
            <th class="score">Skor <span class="col-info" title="Öncelik puanı. Ciddiyet × (1+Bağımlı) × log₂(1+Etkilenen) / Çaba formülüyle hesaplanır. Yüksek skor = önce düzelt.">ℹ</span></th>
            <th class="score-delta-cell">+Yayın <span class="col-info" title="Bu kural düzeltilirse yayın skoru kaç puan artar. Yalnızca blocker/conditional_blocker kurallar için gösterilir. Toplamı = 100 − mevcut yayın skoru.">ℹ</span></th>
            <th class="score-delta-cell">+Kalite <span class="col-info" title="Bu kural düzeltilirse kalite skoru kaç puan artar. Toplamı = 100 − mevcut kalite skoru.">ℹ</span></th>
            <th>Bağımlı <span class="col-info" title="Bu kural düzeltilince kaç başka kural daha kendiliğinden kapanır (bu feed'de ateşlenmiş olan bağımlı kurallar).">ℹ</span></th>
            <th>Çaba <span class="col-info" title="Düzeltmenin iş yükü: 1=kolay (tek alan değişikliği), 5=zor (veri modelinde kapsamlı revizyon).">ℹ</span></th>
          </tr></thead>
          <tbody>${i}</tbody>
        </table>
      </div>
    </div>`}function Ge(e,t,s,o){const i=e.reports.r2.items;if(i.length===0)return'<div class="card"><h2>Tüm Bulgular (R2)</h2><p class="empty">Bulgu yok.</p></div>';const u=`
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
          ${[...new Map(i.filter(l=>t.has(l.notice_id)).map(l=>{const r=t.get(l.notice_id);return[r.rule_id,r.title||r.rule_id]})).entries()].sort(([l],[r])=>l.localeCompare(r)).map(([l,r])=>`<option value="${$(l)}">${$(l)} — ${$(r)}</option>`).join("")}
        </select>
      </label>
      <span id="filter-count" class="filter-count"></span>
    </div>`,a=i.map(l=>{const r=t.get(l.notice_id);if(!r)return"";const n=s.get(r.rule_id),c=n!=null&&n.count>0?n.pubDelta/n.count:null,d=n!=null&&n.count>0?n.qualDelta/n.count:null,h=c!=null&&c>=.005?`<span class="pub-delta">+${c.toFixed(2)}</span>`:'<span class="muted-text">—</span>',b=d!=null&&d>=.005?`<span class="score-delta">+${d.toFixed(2)}</span>`:'<span class="muted-text">—</span>',g=Oe(r,o)?`<button class="map-pin-btn" data-notice-id="${$(r.id)}" title="Haritada göster" aria-label="Haritada göster">📍</button>`:"";return`
      <tr data-severity="${r.severity}" data-class="${r.rule_class}" data-rule="${r.rule_id}">
        <td>${$(l.display_label)}</td>
        <td style="color:${K[r.severity]}">${F[r.severity]}</td>
        <td>${B[r.rule_class]}</td>
        <td>${$(r.message)}</td>
        <td>${r.file?$(r.file):"—"}</td>
        <td>${r.line??"—"}</td>
        <td>${r.field?$(r.field):"—"}</td>
        <td class="score-delta-cell">${h}</td>
        <td class="score-delta-cell">${b}</td>
        <td class="map-btn-cell">${g}</td>
      </tr>`}).join("");return`
    <div class="card" id="r2-card">
      <h2>Tüm Bulgular (R2) <span class="count-badge">${i.length}</span></h2>
      ${u}
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
    </div>`}function $(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function y(e,t,s,o){const i=t.stop_coords[e];if(!i)return null;const p=`<strong>${t.stops[e]??e}</strong>${o?` ${o}`:""}<br><code>${e}</code>`;return{lat:i[0],lon:i[1],label:p,primary:s}}const qe=new Set(["STP_003","STP_004","STP_005","STP_006","STP_007","STP_008","STP_009","STP_010","STP_011","STP_012","STP_013","STP_014","STP_015","STP_016","STP_017","STP_018","STP_019","STP_020","STP_021","STP_022","STP_023","STP_024","STP_025","STP_026","STP_027","STP_028","STP_029","STP_030","GEO_001","GEO_002","GEO_003","GEO_004","GEO_005","GEO_009","GEO_010","GEO_011","GEO_012","GEO_014","PTH_012","PTH_013","PTH_015","PTH_016","PTH_017","PTH_018","PTH_019","LVL_006","SHP_022","VAT_007"]),j=new Set(["TRP_001","TRP_002","TRP_003","TRP_004","TRP_005","TRP_006","TRP_007","TRP_008","TRP_009","TRP_010","TRP_011","TRP_012","TRP_013","TRP_014","TRP_015","TRP_016","TRP_017","TRP_018","TRP_019","TRP_020","FRQ_001","FRQ_002","FRQ_003","FRQ_004","FRQ_005","FRQ_006","FRQ_007","FRQ_008","FRQ_009","STM_001","STM_002","STM_003","STM_004","STM_005","STM_006","STM_007","STM_008","STM_009","STM_010","STM_011","STM_013","STM_015","STM_016","STM_018","STM_019","STM_021","STM_022","STM_023","STM_024","STM_027","STM_028","STM_029","STM_030","STM_031","STM_032","STM_034","OPR_006","OPR_017"]),U=new Set(["RTS_001","RTS_002","RTS_003","RTS_004","RTS_005","RTS_006","RTS_007","RTS_008","RTS_009","RTS_010","RTS_011","RTS_012","RTS_013","RTS_014","RTS_015","RTS_016","RTS_017","OPR_001","OPR_002","OPR_003"]);function Oe(e,t){const s=e.entity_id??"";if(qe.has(e.rule_id))return!!(s&&s in t.stop_coords);if(e.rule_id==="GEO_013")return Object.keys(t.stop_coords).length>0;if(e.rule_id==="SHP_017"){const o=e.message.match(/\(kod: '([^']+)'\)/)?.[1],i=e.details?.ctx_b??"",f=e.details?.ctx_a??"";return!!(o&&o in t.stop_coords)||i.length>0||f.length>0}if(e.rule_id==="STM_014"){const o=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/);return o&&(o[1]in t.stop_coords||o[2]in t.stop_coords)?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_020"){const o=e.details?.stop_a??"",i=e.details?.stop_b??"";return o&&o in t.stop_coords||i&&i in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_025"){const o=e.details?.stop_a??"",i=e.details?.stop_b??"";return o&&o in t.stop_coords||i&&i in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="OPR_015"){const o=e.details?.shape_id??"";return!!(o&&o in t.shape_coords)}if(e.rule_id==="PTH_014"){const o=e.details?.from_station??"",i=e.details?.to_station??"";return!!(o&&o in t.stop_coords&&i&&i in t.stop_coords)}return["OPR_007","OPR_008","STM_017"].includes(e.rule_id)?!!(s&&s in t.trip_shapes):["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"].includes(e.rule_id)?!!(s&&s in t.shape_coords):["STM_012","STM_026","STM_033"].includes(e.rule_id)?!!(s&&s in t.trip_shapes)||!!(s&&s in t.trip_stops):e.rule_id==="VAT_005"?Object.keys(t.stop_coords).length>0:j.has(e.rule_id)?!!(s&&(s in t.trip_shapes||s in t.trip_stops)):U.has(e.rule_id)?!!(s&&(t.route_shapes[s]?.length??0)>0):!1}function De(e,t){const s=e.entity_id??"";if(e.rule_id==="SHP_017"){const a=e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",l=e.details?.shape_id??"",r=(e.details?.ctx_b??"").split(",").filter(Boolean),n=(e.details?.ctx_a??"").split(",").filter(Boolean),c=(e.details?.seq_b??"").split(",").filter(Boolean),d=(e.details?.seq_a??"").split(",").filter(Boolean),h=e.observed_value??"",b=[];for(let v=0;v<r.length;v++){const M=c[v]?`#${c[v]} `:"",H=y(r[v],t,!0,`${M}(önceki)`);H&&b.push(H)}const g=h?`#${h} `:"",m=a?y(a,t,!1,`${g}⚠ sıra bozuk`):null;m&&b.push(m);for(let v=0;v<n.length;v++){const M=d[v]?`#${d[v]} `:"",H=y(n[v],t,!0,`${M}(sonraki)`);H&&b.push(H)}const _=l?t.shape_coords[l]??[]:[],T=[];return _.length>1&&T.push({color:"#f59e0b",label:"Güzergah şekli"}),T.push({color:"#2563eb",label:"Komşu duraklar"}),T.push({color:"#dc2626",label:"Sırası bozuk durak"}),{pins:b,polyline:_.length>1?_:void 0,legendItems:T,showArrows:_.length>1}}if(e.rule_id==="STM_014"){const a=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/),l=a?y(a[1].trim(),t,!0,"(kalkış)"):null,r=a?y(a[2].trim(),t,!1,"(varış)"):null,n=[l,r].filter(b=>b!==null),c=s?t.trip_shapes[s]??"":"",d=c?t.shape_coords[c]??[]:[],h=[];return d.length>1&&h.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>1&&(h.push({color:"#2563eb",label:"Kalkış durağı"}),h.push({color:"#dc2626",label:"Varış durağı"})),{pins:n,polyline:d.length>1?d:void 0,legendItems:h,showArrows:d.length>1}}if(e.rule_id==="STM_020"){const a=e.details?.stop_a?y(e.details.stop_a,t,!0,"(kalkış)"):null,l=e.details?.stop_b?y(e.details.stop_b,t,!1,"(varış)"):null,r=[a,l].filter(h=>h!==null),n=s?t.trip_shapes[s]??"":"",c=n?t.shape_coords[n]??[]:[],d=[];return c.length>1&&d.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&d.push({color:"#2563eb",label:"Kalkış durağı"}),r.length>1&&d.push({color:"#dc2626",label:"Varış durağı"}),{pins:r,polyline:c.length>1?c:void 0,legendItems:d,showArrows:!0}}if(e.rule_id==="SHP_010"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/^\(([^,]+),([^)]+)\)$/),r=[];if(l){const c=parseFloat(l[1]),d=parseFloat(l[2]);!isNaN(c)&&!isNaN(d)&&r.push({lat:c,lon:d,label:`Tekrarlanan koordinat<br><code>(${l[1]}, ${l[2]})</code>`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&n.push({color:"#dc2626",label:"Tekrarlanan nokta"}),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_022"){const a=e.message.match(/durağı '([^']+)' güzergah şeklinin/)?.[1]??"",l=y(s,t,!1,"⚠ belirsiz eşleşme"),r=a?t.shape_coords[a]??[]:[],n=l?[l]:[],c=[];return r.length>1&&c.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&c.push({color:"#dc2626",label:"Belirsiz durak"}),{pins:n,polyline:r.length>1?r:void 0,legendItems:c,showArrows:!0}}if(e.rule_id==="SHP_018"||e.rule_id==="SHP_019"){const a=s?t.shape_coords[s]??[]:[],l=a.length>1?[{color:"#f59e0b",label:"Kullanılmayan güzergah şekli"}]:[];return{pins:[],polyline:a.length>1?a:void 0,legendItems:l,showArrows:!0}}if(e.rule_id==="OPR_007"){const a=e.details?.dup_stop??e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",l=(e.details?.stops??"").split(",").filter(Boolean),r=s?t.trip_shapes[s]??"":"",n=r?t.shape_coords[r]??[]:[],c=new Set,d=[];for(const b of l){if(c.has(b))continue;c.add(b);const g=b===a,m=y(b,t,!g,g?"(tekrar eden)":void 0);m&&(m.small=!g,d.push(m))}const h=[];return n.length>1&&h.push({color:"#f59e0b",label:"Güzergah şekli"}),d.length>1&&h.push({color:"#2563eb",label:"Duraklar"}),a&&h.push({color:"#dc2626",label:"Tekrar eden durak"}),{pins:d,polyline:n.length>1?n:void 0,legendItems:h,showArrows:!0}}if(e.rule_id==="STM_017"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(d=>y(d,t,!0)).filter(d=>d!==null).map(d=>({...d,small:!0})),c=[];return l.length>1&&c.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&c.push({color:"#2563eb",label:"Sefer durakları"}),{pins:n,polyline:l.length>1?l:void 0,legendItems:c,showArrows:!0}}if(e.rule_id==="GEO_006"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/segment\s+(\d+)→(\d+)/),r=[];if(l&&a.length>0){const c=parseInt(l[1],10),d=parseInt(l[2],10),h=a[c],b=a[d];h&&r.push({lat:h[0],lon:h[1],label:`<strong>Atlama başlangıcı</strong><br>Segment ${c}`,primary:!0}),b&&r.push({lat:b[0],lon:b[1],label:`<strong>Atlama bitişi</strong><br>Segment ${d}`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&(n.push({color:"#dc2626",label:"Atlama başlangıcı"}),n.push({color:"#2563eb",label:"Atlama bitişi"})),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!1}}if(e.rule_id==="SHP_020"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/\(([^,]+),([^)]+)\)$/),r=[];if(l){const c=parseFloat(l[1]),d=parseFloat(l[2]);!isNaN(c)&&!isNaN(d)&&r.push({lat:c,lon:d,label:`Tekrarlayan koordinat<br><code>(${l[1]}, ${l[2]})</code>`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&n.push({color:"#dc2626",label:"Tekrarlayan nokta"}),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_009"){const a=s?t.shape_coords[s]??[]:[],l=e.observed_value?.match(/segment (\d+)-(\d+) ∩ (\d+)-(\d+)/),r=[];if(l&&a.length>0){const c=parseInt(l[1],10),d=parseInt(l[2],10),h=parseInt(l[3],10),b=parseInt(l[4],10),g=a[c],m=a[d],_=a[h],T=a[b];g&&r.push({lat:g[0],lon:g[1],label:`Segment ${c}→${d} başlangıcı`,primary:!0}),m&&r.push({lat:m[0],lon:m[1],label:`Segment ${c}→${d} bitişi`,primary:!0}),_&&r.push({lat:_[0],lon:_[1],label:`Segment ${h}→${b} başlangıcı`,primary:!1}),T&&r.push({lat:T[0],lon:T[1],label:`Segment ${h}→${b} bitişi`,primary:!1})}const n=[];return a.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&(n.push({color:"#2563eb",label:"1. kesişen segment"}),n.push({color:"#dc2626",label:"2. kesişen segment"})),{pins:r,polyline:a.length>1?a:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="OPR_008"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=[];l.length>1&&r.push({color:"#9ca3af",label:"Güzergah şekli"});const n=[],c=[],d=new Set;for(let h=0;;h++){const b=e.details?.[`bad_seg_${h}_a`],g=e.details?.[`bad_seg_${h}_b`];if(!b||!g)break;const m=t.stop_coords[b],_=t.stop_coords[g];m&&_&&n.push({coords:[[m[0],m[1]],[_[0],_[1]]],color:"#dc2626",weight:5,zoomTo:h===0}),m&&!d.has(b)&&(d.add(b),c.push({lat:m[0],lon:m[1],label:`<code>${b}</code>`,primary:!0,small:!1})),_&&!d.has(g)&&(d.add(g),c.push({lat:_[0],lon:_[1],label:`<code>${g}</code>`,primary:!0,small:!1}))}return n.length>0&&r.push({color:"#dc2626",label:"Bozuk segment"}),r.push({color:"#2563eb",label:"Segment uçları"}),{pins:c,polyline:l.length>1?l.map(h=>[h[0],h[1]]):void 0,extraPolylines:n,legendItems:r,showArrows:!1}}if(e.rule_id==="STM_025"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",c=e.details?.stop_b??"",d=[];for(const m of r){if(m===n||m===c)continue;const _=y(m,t,!0);_&&d.push({..._,small:!0})}const h=n?y(n,t,!0,"(kalkış)"):null;h&&d.push(h);const b=c?y(c,t,!1,"(varış — çok kısa süre)"):null;b&&d.push(b);const g=[];return l.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),g.push({color:"#dc2626",label:"Kısa süreli segment sonu"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="OPR_015"){const a=e.details?.shape_id??"",l=a?t.shape_coords[a]??[]:[],r=l.length>1?[{color:"#f59e0b",label:"Tek güzergah şekli"}]:[];return{pins:[],polyline:l.length>1?l:void 0,legendItems:r,showArrows:!0}}function o(a){const l=t.shape_trips[a]??"";return(l?t.trip_stops[l]??[]:[]).map(n=>y(n,t,!0)).filter(n=>n!==null).map(n=>({...n,small:!0}))}if(e.rule_id==="SHP_007"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:a.length>1}}if(e.rule_id==="SHP_012"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:!0}}if(e.rule_id==="SHP_014"){const a=s?t.shape_coords[s]??[]:[],l=e.details?.problem_stop??"",r=e.details?.trip_id??"",n=r?t.trip_stops[r]??[]:[],c=[];for(const g of n){if(g===l)continue;const m=y(g,t,!0);m&&c.push({...m,small:!0})}const d=e.details?.endpoint==="start"?"⚠ ilk durak — shape başından uzak":"⚠ son durak — shape bitişinden uzak",h=l?y(l,t,!1,d):null;h&&c.push(h);const b=[];return a.length>1&&b.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&b.push({color:"#2563eb",label:"Sefer durakları"}),h&&b.push({color:"#dc2626",label:"Uçtan uzak durak"}),{pins:c,polyline:a.length>1?a:void 0,legendItems:b,showArrows:!0}}if(e.rule_id==="SHP_015"){const a=s?t.shape_coords[s]??[]:[],l=o(s),r=[];return a.length>1&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),l.length>0&&r.push({color:"#2563eb",label:"Sefer durakları"}),{pins:l,polyline:a.length>1?a:void 0,legendItems:r,showArrows:!0}}if(e.rule_id==="SHP_016"){const a=s?t.shape_coords[s]??[]:[],l=e.details?.first_stop??"",r=e.details?.trip_id??"",n=r?t.trip_stops[r]??[]:[],c=[];for(const b of n){if(b===l)continue;const g=y(b,t,!0);g&&c.push({...g,small:!0})}const d=l?y(l,t,!1,"⚠ ilk durak — shape'in yanlış ucuna düşüyor"):null;d&&c.push(d);const h=[];return a.length>1&&h.push({color:"#f59e0b",label:"Güzergah şekli (ters)"}),n.length>0&&h.push({color:"#2563eb",label:"Sefer durakları"}),d&&h.push({color:"#dc2626",label:"İlk durak (yanlış uçta)"}),{pins:c,polyline:a.length>1?a:void 0,legendItems:h,showArrows:!0}}if(e.rule_id==="STM_012"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",c=e.details?.stop_b??"",d=[];for(const m of r){if(m===n||m===c)continue;const _=y(m,t,!0);_&&d.push({..._,small:!0})}const h=n?y(n,t,!0,"(kalkış — imkansız hız)"):null,b=c?y(c,t,!1,"(varış — imkansız hız)"):null;h&&d.push(h),b&&d.push(b);const g=[];return l.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),g.push({color:"#dc2626",label:"İmkansız hız segmenti"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="STM_026"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],r=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",c=e.details?.stop_b??"",d=[];for(const m of r){if(m===n||m===c)continue;const _=y(m,t,!0);_&&d.push({..._,small:!0})}const h=n?y(n,t,!0,"(kalkış)"):null,b=c?y(c,t,!1,"(varış — çok uzak)"):null;h&&d.push(h),b&&d.push(b);const g=[];return l.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),r.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),g.push({color:"#dc2626",label:"Aşırı uzak segment sonu"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="STM_033"){const a=s?t.trip_shapes[s]??"":"",l=a?t.shape_coords[a]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(d=>y(d,t,!1,"(tek durak — sefer kullanılamaz)")).filter(d=>d!==null),c=[];return l.length>1&&c.push({color:"#f59e0b",label:"Güzergah şekli"}),c.push({color:"#dc2626",label:"Tek durak (kullanılamaz sefer)"}),{pins:n,polyline:l.length>1?l:void 0,legendItems:c,showArrows:l.length>1}}if(e.rule_id==="PTH_014"){const a=e.details?.from_station??"",l=e.details?.to_station??"",r=t.stop_coords[a],n=t.stop_coords[l];if(!r||!n)return defaultMapOptions(e,t);const c=t.stops[a]??a,d=t.stops[l]??l;return{center:[(r[0]+n[0])/2,(r[1]+n[1])/2],zoom:15,markers:[{lat:r[0],lon:r[1],color:"#ef4444",label:c,title:`İstasyon: ${c}`},{lat:n[0],lon:n[1],color:"#3b82f6",label:d,title:`İstasyon: ${d}`}],polylines:[{coords:[[r[0],r[1]],[n[0],n[1]]],color:"#ef4444",dashArray:"6,4",weight:2}],showArrows:!1}}if(e.rule_id==="VAT_005"){const a=new Set((e.details?.isolated_stops??"").split(",").filter(Boolean)),l=new Set;for(const n of Object.values(t.trip_stops))for(const c of n)l.add(c);const r=[];for(const[n,c]of Object.entries(t.stop_coords)){const d=a.has(n),h=l.has(n);if(!d&&!h)continue;const b=t.stops[n]??n;r.push({lat:c[0],lon:c[1],label:d?`<strong>⚠ İzole</strong><br><strong>${b}</strong><br><code>${n}</code>`:`<strong>${b}</strong><br><code>${n}</code>`,primary:!d,small:!d})}return{pins:r,legendItems:[{color:"#2563eb",label:"Bağlı duraklar"},{color:"#dc2626",label:"İzole duraklar"}]}}if(e.rule_id==="VAT_007"){const a=(e.details?.routes??"").split(",").filter(Boolean),l=["#3b82f6","#10b981","#8b5cf6","#f97316","#06b6d4"],r=t.stop_coords[s],n=t.stops[s]??s,c=r?[{lat:r[0],lon:r[1],label:`<strong>Terminal: ${n}</strong><br><code>${s}</code>`,primary:!1}]:[],d=[],h=[];c.length>0&&h.push({color:"#dc2626",label:"Terminal durağı"});let b=0;for(const g of a.slice(0,5)){const m=t.route_shapes[g]??[],_=l[b%l.length];let T=!1;for(const v of m.slice(0,2)){const M=t.shape_coords[v]??[];M.length>1&&(d.push({coords:M,color:_,weight:3,zoomTo:!1}),T=!0)}if(T){const v=t.routes[g]??g;h.push({color:_,label:v}),b++}}return{pins:c,extraPolylines:d,legendItems:h,showArrows:!1}}if(e.rule_id==="GEO_013"){const a=Object.entries(t.stop_coords).map(([l,r])=>({lat:r[0],lon:r[1],label:t.stops[l]?`<strong>${t.stops[l]}</strong><br><code>${l}</code>`:`<code>${l}</code>`,primary:!0,small:!0}));return{pins:a,legendItems:[{color:"#2563eb",label:`${a.length} durak`}]}}if(j.has(e.rule_id)){const a=t.trip_shapes[s]??"",l=a?t.shape_coords[a]??[]:[],r=t.trip_stops[s]??[],n=t.trip_routes[s]??"",c=n?t.routes[n]??"":"",d=r.flatMap((b,g)=>{const m=y(b,t,!0,`#${g+1}`);return m?[{...m,small:!0}]:[]}),h=[];return l.length>1&&h.push({color:"#f59e0b",label:c?`Güzergah şekli — ${c}`:"Güzergah şekli"}),d.length>0&&h.push({color:"#2563eb",label:"Sefer durakları"}),{pins:d,polyline:l.length>1?l:void 0,legendItems:h,showArrows:l.length>1}}if(U.has(e.rule_id)){const a=t.route_shapes[s]??[],l=t.routes[s]??s,r=[];for(const n of a.slice(0,4)){const c=t.shape_coords[n]??[];c.length>1&&r.push({coords:c,color:"#f59e0b",weight:3,zoomTo:r.length===0})}return r.length===0?{pins:[]}:{pins:[],extraPolylines:r,legendItems:[{color:"#f59e0b",label:`Güzergah şekilleri — ${l}`}],showArrows:!1}}const i=t.stop_coords[s];if(!i)return{pins:[]};const[f,p]=i,u=t.stops[s]??s;if(e.rule_id==="STP_029"){const a=e.message.match(/üst istasyonundan \(([^)]+)\)/)?.[1]??"",l=[{lat:f,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],r=a?y(a,t,!1,"(üst istasyon)"):null;r&&l.push(r);const n=l.length>1?[{color:"#2563eb",label:"Durak"},{color:"#dc2626",label:"Üst İstasyon"}]:[];return{pins:l,legendItems:n}}if(e.rule_id==="STP_016"){const a=e.observed_value?.match(/== '([^']+)'/)?.[1]??"",l=a?t.stops[a]??a:"",r=l?`<strong>${u}</strong> = <strong>${l}</strong><br><code>${s}</code> ve <code>${a}</code>`:`<strong>${u}</strong><br><code>${s}</code>`;return{pins:[{lat:f,lon:p,label:r,primary:!0}]}}if(e.rule_id==="STP_017"){const a=e.observed_value?.match(/to '([^']+)'/)?.[1]??"",l=[{lat:f,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],r=a?y(a,t,!1):null;r&&l.push(r);const n=l.length>1?[{color:"#2563eb",label:"Durak 1"},{color:"#dc2626",label:"Durak 2"}]:[];return{pins:l,legendItems:n}}if(e.rule_id==="GEO_009"){const a=e.observed_value?.match(/\(shape '([^']+)'\)/)?.[1]??"",l=a?t.shape_coords[a]??[]:[],r=[{color:"#2563eb",label:"Durak"}];return l.length>0&&r.push({color:"#f59e0b",label:"Güzergah şekli"}),{pins:[{lat:f,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],polyline:l.length>1?l:void 0,legendItems:r}}return{pins:[{lat:f,lon:p,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}]}}function Ke(e,t){if(e.querySelectorAll(".r9-main-row").forEach(u=>{u.addEventListener("click",()=>{const a=u.dataset.idx,l=e.querySelector(`.r9-detail-row[data-for="${a}"]`);if(!l)return;l.hidden=!l.hidden;const r=u.querySelector(".r9-arrow");r&&(r.textContent=l.hidden?"▶":"▼")})}),t){const u=new Map(t.notices.map(a=>[a.id,a]));e.querySelectorAll(".map-pin-btn").forEach(a=>{a.addEventListener("click",l=>{l.stopPropagation();const r=a.dataset.noticeId??"",n=u.get(r);if(!n)return;const c=De(n,t.name_index);if(c.pins.length===0&&!c.polyline)return;const d=new Set(["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"]),h=n.entity_id??"";let b;if(d.has(n.rule_id))b=`şekil: ${h}`;else if(j.has(n.rule_id)){const g=t.name_index.trip_routes[h]??"",m=g?t.name_index.routes[g]??"":"",_=t.name_index.trips[h]??"";b=m?`${m}${_?` — ${_}`:""}`:_||h}else U.has(n.rule_id)?b=t.name_index.routes[h]??h:b=t.name_index.stops[h]??h;Me(`${n.rule_id} — ${b}`,c)})})}const s=e.querySelector("#sev-filter"),o=e.querySelector("#cls-filter"),i=e.querySelector("#rule-filter"),f=e.querySelector("#r2-table");if(!s||!o||!f)return;function p(){const u=s.value,a=o.value,l=i?.value??"";let r=0,n=0;f.querySelectorAll("tbody tr").forEach(d=>{n++;const h=!u||d.dataset.severity===u,b=!a||d.dataset.class===a,g=!l||d.dataset.rule===l,m=h&&b&&g;d.style.display=m?"":"none",m&&r++});const c=e.querySelector("#filter-count");c&&(c.textContent=u||a||l?`${n} noticeden ${r} tanesi`:"")}s.addEventListener("change",p),o.addEventListener("change",p),i?.addEventListener("change",p)}const R={CRITICAL:0,HIGH:1,MEDIUM:2,LOW:3,INFO:4},Be={Stop:"Durak",Trip:"Sefer",Route:"Hat",Shape:"Güzergah",Agency:"İşletici",Service:"Servis",File:"Dosya",Feed:"Feed",Row:"Satır",Field:"Alan",Transfer:"Aktarma",Pathway:"Geçit",Level:"Kat",Translation:"Çeviri",Attribution:"Atıf",Fare:"Ücret"};function Ce(e,t){const s=xe(t.notices),o=t.notices.length;e.innerHTML=`
    <div class="rules-page">
      <div class="rules-summary-bar">
        <span class="rules-summary-text">${o} bulgu · ${s.length} kural</span>
      </div>
      <div class="rules-accordion">
        ${s.map(i=>Ne(i)).join("")}
      </div>
    </div>`,Ie(e)}function xe(e){const t=new Map;for(const s of e){t.has(s.rule_id)||t.set(s.rule_id,{rule_id:s.rule_id,max_severity:s.severity,rule_class:s.rule_class,notices:[]});const o=t.get(s.rule_id);o.notices.push(s),(R[s.severity]??9)<(R[o.max_severity]??9)&&(o.max_severity=s.severity)}return Array.from(t.values()).sort((s,o)=>{const i=(R[s.max_severity]??9)-(R[o.max_severity]??9);return i!==0?i:o.notices.length-s.notices.length})}function Ne(e){const t=K[e.max_severity],s=F[e.max_severity],o=B[e.rule_class]??e.rule_class,i=e.rule_class.toLowerCase(),p=[...e.notices].sort((a,l)=>(R[a.severity]??9)-(R[l.severity]??9)).map(a=>{const l=je(a.message),r=l.length>160?l.slice(0,160)+"…":l,n=K[a.severity]??"#666",c=Be[a.entity_type]??a.entity_type,d=a.entity_id?`<span class="muted-text">${z(c)}</span> <code class="entity-id">${z(a.entity_id)}</code>`:`<span class="muted-text">${z(c)}</span>`;return`
      <tr>
        <td style="color:${n};white-space:nowrap">${F[a.severity]}</td>
        <td>${d}</td>
        <td class="msg-cell">${z(r)}</td>
      </tr>`}).join(""),u=e.notices[0]?.remediation??"";return`
    <div class="rule-acc-section">
      <button class="rule-acc-header" type="button" aria-expanded="false">
        <span class="acc-arrow">▶</span>
        <span class="rule-title-block">
          <code class="rule-code rule-code-lg">${z(e.rule_id)}</code>
          ${u?`<span class="rule-desc-text">${z(u)}</span>`:""}
        </span>
        <span class="badge badge-${i}">${z(o)}</span>
        <span class="rule-sev-badge" style="color:${t}">${z(s)}</span>
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
    </div>`}function Ie(e){e.querySelectorAll(".rule-acc-header").forEach(t=>{t.addEventListener("click",()=>{const o=t.closest(".rule-acc-section").querySelector(".rule-acc-body"),i=t.querySelector(".acc-arrow"),f=!o.hidden;o.hidden=f,i.textContent=f?"▶":"▼",t.setAttribute("aria-expanded",String(!f))})})}function je(e){return e.replace(/\btrip_id\b/g,"sefer").replace(/\bstop_sequence\b/g,"Durak Sırası").replace(/\barrival_time\b/g,"Varış zamanı").replace(/\bdeparture_time\b/g,"Kalkış zamanı").replace(/\bheadway\b/g,"Sefer Aralığı")}function z(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function Ue(e,t,s){e.innerHTML=`
    <section class="page-export-wide">
      <div class="card">
        <h2>Dışa Aktar</h2>
        <p class="export-filename">Kaynak: <code>${P(s)}</code></p>
        <div class="export-actions">
          <button id="btn-export-html" class="btn btn-primary">HTML Rapor İndir</button>
          <button id="btn-export-csv"  class="btn btn-secondary">CSV İndir</button>
          <button id="btn-export-json" class="btn btn-secondary">JSON Ham Veri İndir</button>
          <button id="btn-export-pdf"  class="btn btn-secondary">PDF Olarak Yazdır</button>
        </div>
      </div>
    </section>`,e.querySelector("#btn-export-html").addEventListener("click",()=>Ze(t,s)),e.querySelector("#btn-export-csv").addEventListener("click",()=>Ye(t,s)),e.querySelector("#btn-export-json").addEventListener("click",()=>Ve(t,s)),e.querySelector("#btn-export-pdf").addEventListener("click",()=>Qe(t,s))}function W(e){const t=e==null?"":String(e);return t.includes(",")||t.includes('"')||t.includes(`
`)?`"${t.replace(/"/g,'""')}"`:t}function Ye(e,t){const s=["Kural","Önem","Sınıf","Mesaj","Varlık ID","Dosya","Satır"],o=e.notices.map(u=>[u.rule_id,F[u.severity]??u.severity,B[u.rule_class]??u.rule_class,u.message,u.entity_id??"",u.file??"",u.line!=null?String(u.line):""].map(W).join(",")),f="\uFEFF"+[s.map(W).join(","),...o].join(`\r
`),p=new Blob([f],{type:"text/csv; charset=utf-8"});Y(p,t.replace(/\.zip$/i,"-report.csv"))}function Ve(e,t){const s=new Blob([JSON.stringify(e,null,2)],{type:"application/json"});Y(s,t.replace(/\.zip$/i,"-report.json"))}function ne(e,t){const{r1:s,r5:o}=e.reports,i=s.publishable?s.conditional?"Koşullu Yayına Uygun":"Yayına Uygun":"Yayınlanması Tavsiye Edilmez",f=e.notices.map(p=>`
    <tr>
      <td>${P(p.rule_id)}</td>
      <td>${F[p.severity]}</td>
      <td>${B[p.rule_class]}</td>
      <td>${P(p.message)}</td>
      <td>${p.entity_id?P(p.entity_id):""}</td>
      <td>${p.file?P(p.file):""}</td>
      <td>${p.line??""}</td>
    </tr>`).join("");return`<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8"/>
  <title>GTFS Doğrulama Raporu - ${P(t)}</title>
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
  <p style="color:#64748b;font-size:.9rem">Kaynak: ${P(t)}</p>

  <div class="summary">
    <div class="kpi">
      <span class="kpi-value">${i}</span>
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
</html>`}function Ze(e,t){const s=ne(e,t),o=new Blob([s],{type:"text/html; charset=utf-8"});Y(o,t.replace(/\.zip$/i,"-report.html"))}function Qe(e,t){const s=ne(e,t),o=window.open("","_blank");if(!o){alert("Açılır pencere engellendi. Tarayıcı ayarlarını kontrol edin.");return}o.document.write(s),o.document.close(),o.focus(),o.print()}function Y(e,t){const s=URL.createObjectURL(e),o=document.createElement("a");o.href=s,o.download=t,o.click(),URL.revokeObjectURL(s)}function P(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}const J={domain:"Rapor",fix:"Ayrıntı ve Düzeltme",rules:"Kategori Bazlı",export:"Dışa Aktar"};function We(){localStorage.getItem("gtfs-theme")==="dark"&&document.documentElement.classList.add("dark")}function X(){const e=document.documentElement.classList.toggle("dark");localStorage.setItem("gtfs-theme",e?"dark":"light"),Je()}function Je(){const e=document.documentElement.classList.contains("dark");document.querySelectorAll(".dark-toggle").forEach(t=>{t.textContent=e?"☀":"☾",t.title=e?"Açık mod":"Koyu mod"})}function ee(){const e=document.documentElement.classList.contains("dark");return`<button class="btn btn-ghost dark-toggle" title="${e?"Açık mod":"Koyu mod"}">${e?"☀":"☾"}</button>`}function E(){const e=N(),t=document.getElementById("app");if(e.page==="upload"||!e.result){t.innerHTML=`
      <header class="app-header">
        <h1>GTFS Analyzer</h1>
        <span style="flex:1"></span>
        ${ee()}
      </header>
      <main id="page-root"></main>`,t.querySelector(".dark-toggle").addEventListener("click",X),fe(document.getElementById("page-root"));return}const s=Object.keys(J).map(i=>`
      <button class="nav-btn ${e.page===i?"active":""}" data-page="${i}">
        ${J[i]}
      </button>`).join("");t.innerHTML=`
    <header class="app-header">
      <h1>GTFS Validator</h1>
      <span class="header-filename">${Xe(e.fileName)}</span>
      <button id="btn-back" class="btn btn-ghost">← Ana Sayfa</button>
      <button id="btn-new" class="btn btn-secondary">Yeni GTFS Yükle</button>
      ${ee()}
    </header>
    <nav class="app-nav">${s}</nav>
    <main id="page-root"></main>`,t.querySelector("#btn-back").addEventListener("click",()=>{q("upload"),E()}),t.querySelector("#btn-new").addEventListener("click",()=>{q("upload"),E()}),t.querySelector(".dark-toggle").addEventListener("click",X),t.querySelectorAll(".nav-btn").forEach(i=>{i.addEventListener("click",()=>{q(i.dataset.page),E()})});const o=document.getElementById("page-root");switch(e.page){case"domain":ke(o,e.result);break;case"fix":Fe(o,e.result),Ke(o,e.result);break;case"rules":Ce(o,e.result);break;case"export":Ue(o,e.result,e.fileName);break}}function Xe(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}We();E();
