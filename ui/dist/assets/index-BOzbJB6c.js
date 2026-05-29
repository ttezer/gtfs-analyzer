(function(){const t=document.createElement("link").relList;if(t&&t.supports&&t.supports("modulepreload"))return;for(const p of document.querySelectorAll('link[rel="modulepreload"]'))i(p);new MutationObserver(p=>{for(const h of p)if(h.type==="childList")for(const f of h.addedNodes)f.tagName==="LINK"&&f.rel==="modulepreload"&&i(f)}).observe(document,{childList:!0,subtree:!0});function s(p){const h={};return p.integrity&&(h.integrity=p.integrity),p.referrerPolicy&&(h.referrerPolicy=p.referrerPolicy),p.crossOrigin==="use-credentials"?h.credentials="include":p.crossOrigin==="anonymous"?h.credentials="omit":h.credentials="same-origin",h}function i(p){if(p.ep)return;p.ep=!0;const h=s(p);fetch(p.href,h)}})();const O={page:"upload",result:null,configDelta:sessionStorage.getItem("gtfs-config-delta")??"",fileName:""};function X(){return O}function we(e,t){O.result=e,O.fileName=t,O.page="domain"}function K(e){O.page=e}function _e(e){O.configDelta=e,sessionStorage.setItem("gtfs-config-delta",e)}let D=localStorage.getItem("gtfs-locale")==="en"?"en":"tr";function ie(){return D}function re(e){D=e,localStorage.setItem("gtfs-locale",e),ve()}const me={tr:{CRITICAL:"KRİTİK",HIGH:"YÜKSEK",MEDIUM:"ORTA",LOW:"DÜŞÜK",INFO:"BİLGİ"},en:{CRITICAL:"Critical",HIGH:"High",MEDIUM:"Medium",LOW:"Low",INFO:"Info"}},x={...me.tr},ge={tr:{SPEC:"GTFS Geçerliliği",INTEROP:"GTFS Uyumluluğu",QUALITY:"GTFS Kalitesi",ANALYTICS:"GTFS Analitiği"},en:{SPEC:"GTFS Validity",INTEROP:"GTFS Interop.",QUALITY:"GTFS Quality",ANALYTICS:"GTFS Analytics"}},P={...ge.tr},be={tr:{ZipUnreadable:"ZIP dosyası açılamadı",Utf8Critical:"UTF-8 kodlama hatası",NoRequiredFiles:"Zorunlu dosyalar eksik",CsvMalformed:"CSV biçim hatası",ResourceLimit:"Notice limiti aşıldı",InvalidInput:"Geçersiz girdi"},en:{ZipUnreadable:"ZIP file could not be opened",Utf8Critical:"UTF-8 encoding error",NoRequiredFiles:"Required files missing",CsvMalformed:"CSV format error",ResourceLimit:"Notice limit exceeded",InvalidInput:"Invalid input"}},ye={...be.tr},V={CRITICAL:"var(--color-critical)",HIGH:"var(--color-high)",MEDIUM:"var(--color-medium)",LOW:"var(--color-low)",INFO:"var(--color-info)"},oe={tr:{"nav.report":"Rapor","nav.fix":"Ayrıntı ve Düzeltme","nav.rules":"Kategori Bazlı","nav.export":"Dışa Aktar","header.title":"GTFS Analyzer","header.back":"← Ana Sayfa","header.new":"Yeni GTFS Yükle","lang.switch":"EN","theme.light":"Açık mod","theme.dark":"Koyu mod","upload.files_section":"GTFS Dosyaları","upload.stages_section":"İşlem Aşamaları","upload.no_files":"Henüz dosya yüklenmedi","upload.drag":"ZIP sürükleyin","upload.load":"GTFS Yükle","upload.load_another":"Farklı GTFS Yükle","upload.show_report":"Raporu Göster →","upload.waiting":"bekliyor","upload.running":"çalışıyor…","upload.row_unit":"satır","upload.settings_title":"Analiz Kriterleri","upload.settings_modified":"Değiştirildi","upload.apply":"Uygula","upload.reset":"Sıfırla","upload.settings_saved":"Ayarlar kaydedildi. Yeni ZIP yükleyince uygulanır.","upload.settings_reset":"Varsayılanlara döndürüldü.","upload.error_zip":"Yalnızca .zip uzantılı dosyalar kabul edilir.","upload.error_size":"Dosya çok büyük ({mb} MB). Maksimum izin verilen boyut: 512 MB.","upload.default_for":"varsayılan: {val}","stage.K1":"K1 — Dosya Ayrıştırma","stage.K2":"K2 — Kural Doğrulama","stage.K3":"K3 — Varlık Haritası","stage.K4":"K4 — Çapraz Kontrol","stage.K5":"K5 — Türetilmiş Veri","stage.K6":"K6 — İstatistik Analizi","stage.K7":"K7 — Rapor Oluşturma","file.stops.txt":"durak","file.routes.txt":"hat","file.trips.txt":"sefer","file.stop_times.txt":"geçiş zamanı","file.shapes.txt":"güzergah","file.agency.txt":"işletici","file.calendar.txt":"takvim","file.calendar_dates.txt":"özel gün","file.frequencies.txt":"frekans","file.transfers.txt":"aktarma","file.pathways.txt":"geçit","file.levels.txt":"kat","file.attributions.txt":"atıf","file.translations.txt":"çeviri","cfg.max_speed_bus_kmh.label":"Maks. Otobüs Hızı","cfg.max_speed_bus_kmh.unit":"km/s","cfg.max_speed_bus_kmh.desc":"Otobüs seferleri için maksimum izin verilen hız","cfg.max_speed_tram_kmh.label":"Maks. Tramvay Hızı","cfg.max_speed_tram_kmh.unit":"km/s","cfg.max_speed_tram_kmh.desc":"Tramvay seferleri için maksimum izin verilen hız","cfg.max_speed_metro_kmh.label":"Maks. Metro Hızı","cfg.max_speed_metro_kmh.unit":"km/s","cfg.max_speed_metro_kmh.desc":"Metro seferleri için maksimum izin verilen hız","cfg.max_speed_rail_kmh.label":"Maks. Demiryolu Hızı","cfg.max_speed_rail_kmh.unit":"km/s","cfg.max_speed_rail_kmh.desc":"Demiryolu seferleri için maksimum izin verilen hız","cfg.max_speed_ferry_kmh.label":"Maks. Feribot Hızı","cfg.max_speed_ferry_kmh.unit":"km/s","cfg.max_speed_ferry_kmh.desc":"Feribot seferleri için maksimum izin verilen hız","cfg.max_speed_cablecar_kmh.label":"Maks. Teleferik Hızı","cfg.max_speed_cablecar_kmh.unit":"km/s","cfg.max_speed_cablecar_kmh.desc":"Teleferik/füniküler için maksimum izin verilen hız","cfg.min_transfer_time_sec.label":"Min. Aktarma Süresi","cfg.min_transfer_time_sec.unit":"sn","cfg.min_transfer_time_sec.desc":"Transferler için minimum bağlantı süresi","cfg.max_transfer_distance_m.label":"Maks. Aktarma Mesafesi","cfg.max_transfer_distance_m.unit":"m","cfg.max_transfer_distance_m.desc":"Transfer geçerli sayılmak için maksimum mesafe","cfg.max_shape_jump_km.label":"Maks. Güzergah Sıçraması","cfg.max_shape_jump_km.unit":"km","cfg.max_shape_jump_km.desc":"Art arda güzergah noktaları arasındaki maksimum mesafe","cfg.stop_too_close_m.label":"Çok Yakın Durak Eşiği","cfg.stop_too_close_m.unit":"m","cfg.stop_too_close_m.desc":"Bu mesafeden yakın duraklar tekrar sayılır","cfg.stop_far_from_shape_m.label":"Durağın Güzergah'a Uzaklığı","cfg.stop_far_from_shape_m.unit":"m","cfg.stop_far_from_shape_m.desc":"Durağın güzergahından en fazla bu kadar uzakta olabilir","cfg.stop_far_from_parent_m.label":"Üst İstasyona Uzaklık Eşiği","cfg.stop_far_from_parent_m.unit":"m","cfg.stop_far_from_parent_m.desc":"Durak, üst istasyonundan en fazla bu kadar uzakta olabilir","cfg.feed_expiry_warning_days.label":"Son Kullanma Uyarısı","cfg.feed_expiry_warning_days.unit":"gün","cfg.feed_expiry_warning_days.desc":"Feed bu kadar günden az kalmışsa uyarı üretilir","cfg.service_gap_days.label":"Servis Boşluğu Eşiği","cfg.service_gap_days.unit":"gün","cfg.service_gap_days.desc":"Bu günden uzun servis kesintisi işaretlenir","cfg.max_trip_duration_hours.label":"Maks. Sefer Süresi","cfg.max_trip_duration_hours.unit":"saat","cfg.max_trip_duration_hours.desc":"Tek bir seferin maksimum süresi","cfg.min_trip_duration_sec.label":"Min. Sefer Süresi","cfg.min_trip_duration_sec.unit":"sn","cfg.min_trip_duration_sec.desc":"Tek bir seferin minimum süresi","cfg.max_headway_warning_min.label":"Maks. Sefer Aralığı","cfg.max_headway_warning_min.unit":"dk","cfg.max_headway_warning_min.desc":"Bu dakikadan uzun aralık uyarı üretir","cfg.bunching_threshold_min.label":"Sıkışma Eşiği","cfg.bunching_threshold_min.unit":"dk","cfg.bunching_threshold_min.desc":"Bu dakikadan kısa aralık sıkışma sayılır","score.pub":"Yayın Skoru","score.qual":"Kalite Skoru","class.SPEC":"GTFS Geçerliliği","class.INTEROP":"GTFS Uyumluluğu","class.QUALITY":"GTFS Kalitesi","class.ANALYTICS":"GTFS Analitiği","domain.pub_score":"Yayın Skoru","domain.pub_hint":"SPEC + INTEROP blocker temizliği","domain.qual_score":"Kalite Skoru","domain.qual_hint":"Tüm sınıfların ağırlıklı ortalaması","domain.r1_blocked_title":"Yayınlanması Tavsiye Edilmez","domain.r1_blocked_desc":"{n} kritik engelleyici bulgu mevcut — feed yayına alınamaz.","domain.r1_conditional_title":"Koşullu Yayına Uygun","domain.r1_conditional_desc":"{n} koşullu engelleyici mevcut — bazı uygulamalar bu feed'i reddedebilir.","domain.r1_ok_title":"Yayına Uygun","domain.r1_ok_desc":"Engelleyici bulgu yok.","domain.subscores_title":"Kalite Skoru Bileşenleri","domain.pie_title":"Hata Dağılımı","domain.pie_center":"hata","domain.sev.CRITICAL":"Kritik","domain.sev.HIGH":"Yüksek","domain.sev.MEDIUM":"Orta","domain.sev.LOW":"Düşük","domain.sev.INFO":"Bilgi","domain.metrics_title":"Feed Metrikleri","domain.metric.stops":"Durak","domain.metric.routes":"Hat","domain.metric.trips":"Sefer","domain.metric.shapes":"Güzergah","domain.metric.service_days":"Aktif Servis Günü","domain.metric.avg_trips":"Ort. Günlük Sefer","domain.table.file":"Dosya","domain.table.rows":"Satır","domain.table.size":"Boyut","label.blocker":"Engelleyici","label.conditional_blocker":"Koşullu Engelleyici","label.interop":"Uyumluluk","label.propagation":"Zincirleme","label.quick-win":"Kolay Düzeltme","label.quality":"Kalite Öncelikli","label.widespread":"Yaygın","label.analytics":"Analitik","label.hard":"Zor","label.single":"Tek Örnek","label.high-impact":"Yüksek Etki","fix.r9_title":"Düzeltme Kuyruğu (R9)","fix.r9_hint":"Satıra tıklayarak açıklama ve çözüm önerisini görebilirsiniz.","fix.r9_empty":"Düzeltilecek bulgu yok.","fix.r9_message":"Mesaj:","fix.r9_remediation":"Çözüm:","fix.th.rule":"Kural","fix.th.severity":"Önem","fix.th.label":"Etiket","fix.th.count":"Hata","fix.th.count.tip":"Bu kuraldan kaç hata üretildi — sorunun yaygınlığını gösterir.","fix.th.total":"Toplam","fix.th.total.tip":"Kural başına notice sınırı (cap) uygulandığında gerçek ihlal sayısı. Sınıra çarpmayan kurallarda bu sütun boş kalır.","fix.th.score":"Skor","fix.th.score.tip":"Öncelik puanı. Ciddiyet × (1+Bağımlı) × log₂(1+Etkilenen) / Çaba. Yüksek skor = önce düzelt.","fix.th.pub":"+Yayın","fix.th.pub.tip":"Bu kural düzeltilirse yayın skoru kaç puan artar. Toplamı = 100 − mevcut yayın skoru.","fix.th.quality":"+Kalite","fix.th.quality.tip":"Bu kural düzeltilirse kalite skoru kaç puan artar. Toplamı = 100 − mevcut kalite skoru.","fix.th.dependent":"Bağımlı","fix.th.dependent.tip":"Bu kural düzeltilince kaç başka kural daha kendiliğinden kapanır.","fix.th.effort":"Çaba","fix.th.effort.tip":"Düzeltme iş yükü: 1=kolay (tek alan), 5=zor (veri modeli revizyonu).","fix.r2_title":"Tüm Bulgular (R2)","fix.r2_empty":"Bulgu yok.","fix.filter.severity":"Önem filtrele:","fix.filter.class":"Sınıf filtrele:","fix.filter.rule":"Kural filtrele:","fix.filter.all":"Tümü","fix.filter.count":"{total} noticeden {visible} tanesi","fix.cap_warning":"Bu kural cap'e çarptı — raporda yalnızca {shown} notice gösterilmektedir, gerçek toplam: {total}.","fix.r2.th.rule":"Kural","fix.r2.th.severity":"Önem","fix.r2.th.class":"Sınıf","fix.r2.th.message":"Mesaj","fix.r2.th.file":"Dosya","fix.r2.th.row":"Satır","fix.r2.th.field":"Alan","fix.r2.th.pub":"+Yayın","fix.r2.th.pub.tip":"Bu hatayı düzeltmenin yayın skoruna katkısı. Toplamı = 100 − mevcut yayın skoru.","fix.r2.th.quality":"+Kalite","fix.r2.th.quality.tip":"Bu hatayı düzeltmenin kalite skoruna katkısı. Toplamı = 100 − mevcut kalite skoru.","fix.map_btn":"Haritada göster","fix.map.shape":"şekil","export.title":"Dışa Aktar","export.source":"Kaynak:","export.html":"HTML Rapor İndir","export.csv":"CSV İndir","export.json":"JSON Ham Veri İndir","export.pdf":"PDF Olarak Yazdır","export.popup_blocked":"Açılır pencere engellendi. Tarayıcı ayarlarını kontrol edin.","export.csv.rule":"Kural","export.csv.severity":"Önem","export.csv.class":"Sınıf","export.csv.message":"Mesaj","export.csv.entity_id":"Varlık ID","export.csv.file":"Dosya","export.csv.row":"Satır","export.html.doc_title":"GTFS Doğrulama Raporu","export.html.h1":"GTFS Doğrulama Raporu","export.html.publishability":"Yayınlanabilirlik","export.html.qual_score":"Kalite Skoru","export.html.pub_score":"Yayın Skoru","export.html.total":"Toplam Bulgu","export.html.h2_findings":"Bulgular","export.html.publishable_ok":"Yayına Uygun","export.html.publishable_cond":"Koşullu Yayına Uygun","export.html.publishable_blocked":"Yayınlanması Tavsiye Edilmez","export.html.breakdown":"GTFS Geçerliliği {spec} · GTFS Uyumluluğu {interop} · GTFS Kalitesi {quality} · GTFS Analitiği {analytics}","export.html.th.rule":"Kural","export.html.th.severity":"Önem","export.html.th.class":"Sınıf","export.html.th.message":"Mesaj","export.html.th.entity_id":"Varlık ID","export.html.th.file":"Dosya","export.html.th.row":"Satır","rules.summary":"{total} bulgu · {rules} kural","rules.th.severity":"Önem","rules.th.entity":"Varlık","rules.th.message":"Mesaj","entity.Stop":"Durak","entity.Trip":"Sefer","entity.Route":"Hat","entity.Shape":"Güzergah","entity.Agency":"İşletici","entity.Service":"Servis","entity.File":"Dosya","entity.Feed":"Feed","entity.Row":"Satır","entity.Field":"Alan","entity.Transfer":"Aktarma","entity.Pathway":"Geçit","entity.Level":"Kat","entity.Translation":"Çeviri","entity.Attribution":"Atıf","entity.Fare":"Ücret"},en:{"nav.report":"Report","nav.fix":"Details & Fix","nav.rules":"By Category","nav.export":"Export","header.title":"GTFS Analyzer","header.back":"← Home","header.new":"Load New GTFS","lang.switch":"TR","theme.light":"Light mode","theme.dark":"Dark mode","upload.files_section":"GTFS Files","upload.stages_section":"Processing Stages","upload.no_files":"No files loaded yet","upload.drag":"Drop ZIP file","upload.load":"Load GTFS","upload.load_another":"Load Another GTFS","upload.show_report":"Show Report →","upload.waiting":"waiting","upload.running":"running…","upload.row_unit":"row","upload.settings_title":"Analysis Settings","upload.settings_modified":"Modified","upload.apply":"Apply","upload.reset":"Reset","upload.settings_saved":"Settings saved. Will apply on next ZIP upload.","upload.settings_reset":"Reset to defaults.","upload.error_zip":"Only .zip files are accepted.","upload.error_size":"File too large ({mb} MB). Maximum allowed size: 512 MB.","upload.default_for":"default: {val}","stage.K1":"K1 — File Parsing","stage.K2":"K2 — Field Validation","stage.K3":"K3 — Entity Graph","stage.K4":"K4 — Cross-Reference","stage.K5":"K5 — Derived Data","stage.K6":"K6 — Statistical Analysis","stage.K7":"K7 — Report Generation","file.stops.txt":"stop","file.routes.txt":"route","file.trips.txt":"trip","file.stop_times.txt":"stop time","file.shapes.txt":"shape","file.agency.txt":"agency","file.calendar.txt":"calendar","file.calendar_dates.txt":"exception","file.frequencies.txt":"frequency","file.transfers.txt":"transfer","file.pathways.txt":"pathway","file.levels.txt":"level","file.attributions.txt":"attribution","file.translations.txt":"translation","cfg.max_speed_bus_kmh.label":"Max. Bus Speed","cfg.max_speed_bus_kmh.unit":"km/h","cfg.max_speed_bus_kmh.desc":"Maximum allowed speed for bus services","cfg.max_speed_tram_kmh.label":"Max. Tram Speed","cfg.max_speed_tram_kmh.unit":"km/h","cfg.max_speed_tram_kmh.desc":"Maximum allowed speed for tram services","cfg.max_speed_metro_kmh.label":"Max. Metro Speed","cfg.max_speed_metro_kmh.unit":"km/h","cfg.max_speed_metro_kmh.desc":"Maximum allowed speed for metro services","cfg.max_speed_rail_kmh.label":"Max. Rail Speed","cfg.max_speed_rail_kmh.unit":"km/h","cfg.max_speed_rail_kmh.desc":"Maximum allowed speed for rail services","cfg.max_speed_ferry_kmh.label":"Max. Ferry Speed","cfg.max_speed_ferry_kmh.unit":"km/h","cfg.max_speed_ferry_kmh.desc":"Maximum allowed speed for ferry services","cfg.max_speed_cablecar_kmh.label":"Max. Cable Car Speed","cfg.max_speed_cablecar_kmh.unit":"km/h","cfg.max_speed_cablecar_kmh.desc":"Maximum allowed speed for cable car/funicular","cfg.min_transfer_time_sec.label":"Min. Transfer Time","cfg.min_transfer_time_sec.unit":"sec","cfg.min_transfer_time_sec.desc":"Minimum connection time for transfers","cfg.max_transfer_distance_m.label":"Max. Transfer Distance","cfg.max_transfer_distance_m.unit":"m","cfg.max_transfer_distance_m.desc":"Maximum distance to count as a valid transfer","cfg.max_shape_jump_km.label":"Max. Shape Jump","cfg.max_shape_jump_km.unit":"km","cfg.max_shape_jump_km.desc":"Maximum distance between consecutive shape points","cfg.stop_too_close_m.label":"Too-Close Stop Threshold","cfg.stop_too_close_m.unit":"m","cfg.stop_too_close_m.desc":"Stops closer than this are flagged as duplicates","cfg.stop_far_from_shape_m.label":"Stop-to-Shape Distance","cfg.stop_far_from_shape_m.unit":"m","cfg.stop_far_from_shape_m.desc":"Maximum allowed distance from stop to its shape","cfg.stop_far_from_parent_m.label":"Stop-to-Parent Distance","cfg.stop_far_from_parent_m.unit":"m","cfg.stop_far_from_parent_m.desc":"Maximum distance from stop to its parent station","cfg.feed_expiry_warning_days.label":"Expiry Warning","cfg.feed_expiry_warning_days.unit":"days","cfg.feed_expiry_warning_days.desc":"Warning triggered when fewer than this many days remain","cfg.service_gap_days.label":"Service Gap Threshold","cfg.service_gap_days.unit":"days","cfg.service_gap_days.desc":"Service gaps longer than this are flagged","cfg.max_trip_duration_hours.label":"Max. Trip Duration","cfg.max_trip_duration_hours.unit":"hours","cfg.max_trip_duration_hours.desc":"Maximum duration of a single trip","cfg.min_trip_duration_sec.label":"Min. Trip Duration","cfg.min_trip_duration_sec.unit":"sec","cfg.min_trip_duration_sec.desc":"Minimum duration of a single trip","cfg.max_headway_warning_min.label":"Max. Headway Warning","cfg.max_headway_warning_min.unit":"min","cfg.max_headway_warning_min.desc":"Headways longer than this trigger a warning","cfg.bunching_threshold_min.label":"Bunching Threshold","cfg.bunching_threshold_min.unit":"min","cfg.bunching_threshold_min.desc":"Intervals shorter than this are flagged as bunching","score.pub":"Publish Score","score.qual":"Quality Score","class.SPEC":"GTFS Validity","class.INTEROP":"GTFS Interoperability","class.QUALITY":"GTFS Quality","class.ANALYTICS":"GTFS Analytics","domain.pub_score":"Publish Score","domain.pub_hint":"SPEC + INTEROP blocker clearance","domain.qual_score":"Quality Score","domain.qual_hint":"Weighted average of all classes","domain.r1_blocked_title":"Not Recommended for Publishing","domain.r1_blocked_desc":"{n} critical blocker(s) found — feed cannot be published.","domain.r1_conditional_title":"Conditionally Publishable","domain.r1_conditional_desc":"{n} conditional blocker(s) — some applications may reject this feed.","domain.r1_ok_title":"Ready to Publish","domain.r1_ok_desc":"No blockers found.","domain.subscores_title":"Quality Score Components","domain.pie_title":"Error Distribution","domain.pie_center":"errors","domain.sev.CRITICAL":"Critical","domain.sev.HIGH":"High","domain.sev.MEDIUM":"Medium","domain.sev.LOW":"Low","domain.sev.INFO":"Info","domain.metrics_title":"Feed Metrics","domain.metric.stops":"Stops","domain.metric.routes":"Routes","domain.metric.trips":"Trips","domain.metric.shapes":"Shapes","domain.metric.service_days":"Active Service Days","domain.metric.avg_trips":"Avg. Daily Trips","domain.table.file":"File","domain.table.rows":"Rows","domain.table.size":"Size","label.blocker":"Blocker","label.conditional_blocker":"Conditional Blocker","label.interop":"Interop","label.propagation":"Propagation","label.quick-win":"Quick Win","label.quality":"Quality Priority","label.widespread":"Widespread","label.analytics":"Analytics","label.hard":"Hard","label.single":"Single Instance","label.high-impact":"High Impact","fix.r9_title":"Fix Queue (R9)","fix.r9_hint":"Click a row to see details and remediation.","fix.r9_empty":"No issues to fix.","fix.r9_message":"Message:","fix.r9_remediation":"Remediation:","fix.th.rule":"Rule","fix.th.severity":"Severity","fix.th.label":"Label","fix.th.count":"Count","fix.th.count.tip":"How many errors were generated — shows prevalence.","fix.th.total":"Total","fix.th.total.tip":"Actual violation count when notice cap was applied. Empty if not capped.","fix.th.score":"Score","fix.th.score.tip":"Priority score. Severity × (1+Dependent) × log₂(1+Count) / Effort. Higher = fix first.","fix.th.pub":"+Pub","fix.th.pub.tip":"Publish score gain if this rule is fixed. Sum = 100 − current publish score.","fix.th.quality":"+Quality","fix.th.quality.tip":"Quality score gain if this rule is fixed. Sum = 100 − current quality score.","fix.th.dependent":"Dependent","fix.th.dependent.tip":"How many other active rules close automatically if this one is fixed.","fix.th.effort":"Effort","fix.th.effort.tip":"Fix effort: 1=easy (single field), 5=hard (data model revision).","fix.r2_title":"All Findings (R2)","fix.r2_empty":"No findings.","fix.filter.severity":"Filter by severity:","fix.filter.class":"Filter by class:","fix.filter.rule":"Filter by rule:","fix.filter.all":"All","fix.filter.count":"{visible} of {total} notices","fix.cap_warning":"This rule hit the cap — only {shown} notices shown, actual total: {total}.","fix.r2.th.rule":"Rule","fix.r2.th.severity":"Severity","fix.r2.th.class":"Class","fix.r2.th.message":"Message","fix.r2.th.file":"File","fix.r2.th.row":"Row","fix.r2.th.field":"Field","fix.r2.th.pub":"+Pub","fix.r2.th.pub.tip":"Publish score contribution from fixing this error. Sum = 100 − current publish score.","fix.r2.th.quality":"+Quality","fix.r2.th.quality.tip":"Quality score contribution from fixing this error. Sum = 100 − current quality score.","fix.map_btn":"Show on map","fix.map.shape":"shape","export.title":"Export","export.source":"Source:","export.html":"Download HTML Report","export.csv":"Download CSV","export.json":"Download Raw JSON","export.pdf":"Print as PDF","export.popup_blocked":"Popup blocked. Check your browser settings.","export.csv.rule":"Rule","export.csv.severity":"Severity","export.csv.class":"Class","export.csv.message":"Message","export.csv.entity_id":"Entity ID","export.csv.file":"File","export.csv.row":"Row","export.html.doc_title":"GTFS Validation Report","export.html.h1":"GTFS Validation Report","export.html.publishability":"Publishability","export.html.qual_score":"Quality Score","export.html.pub_score":"Publish Score","export.html.total":"Total Findings","export.html.h2_findings":"Findings","export.html.publishable_ok":"Ready to Publish","export.html.publishable_cond":"Conditionally Publishable","export.html.publishable_blocked":"Not Recommended for Publishing","export.html.breakdown":"GTFS Validity {spec} · GTFS Interop. {interop} · GTFS Quality {quality} · GTFS Analytics {analytics}","export.html.th.rule":"Rule","export.html.th.severity":"Severity","export.html.th.class":"Class","export.html.th.message":"Message","export.html.th.entity_id":"Entity ID","export.html.th.file":"File","export.html.th.row":"Row","rules.summary":"{total} findings · {rules} rules","rules.th.severity":"Severity","rules.th.entity":"Entity","rules.th.message":"Message","entity.Stop":"Stop","entity.Trip":"Trip","entity.Route":"Route","entity.Shape":"Shape","entity.Agency":"Agency","entity.Service":"Service","entity.File":"File","entity.Feed":"Feed","entity.Row":"Row","entity.Field":"Field","entity.Transfer":"Transfer","entity.Pathway":"Pathway","entity.Level":"Level","entity.Translation":"Translation","entity.Attribution":"Attribution","entity.Fare":"Fare"}};function r(e,t){let s=oe[D][e]??oe.tr[e]??e;if(t)for(const[i,p]of Object.entries(t))s=s.split(`{${i}}`).join(String(p));return s}function ve(){Object.assign(x,me[D]),Object.assign(P,ge[D]),Object.assign(ye,be[D])}ve();const H=new Map;let Me=1,Z=ke();function ke(){const e=new Worker(new URL(""+new URL("validator-worker-CKmSSyZF.js",import.meta.url).href,import.meta.url),{type:"module"});return e.onmessage=Le,e.onerror=Pe,e}function Le(e){const t=e.data,s=H.get(t.id);if(s){if(t.type==="file-list"){s.callbacks?.onFileList?.(t.files);return}if(t.type==="file-done"){s.callbacks?.onFileDone?.(t.name,t.rows,t.bytes);return}if(t.type==="stage"){s.callbacks?.onStageDone?.(t.stage,t.elapsed_ms);return}t.type==="result"&&(H.delete(t.id),t.ok?s.resolve(t.result):s.reject(t.error))}}function Pe(e){const t={code:"ResourceLimit",message:e.message||"Arka plan doğrulama işçisi beklenmedik şekilde durdu."};for(const[,s]of H)s.reject(t);H.clear(),Z.terminate(),Z=ke()}const Re=5*60*1e3;function Ae(e,t){return setTimeout(()=>{H.has(e)&&(H.delete(e),t({code:"ResourceLimit",message:"Doğrulama 5 dakika içinde tamamlanamadı. Daha küçük bir feed deneyin."}))},Re)}function ze(e,t,s){const i=Me++;return new Promise((p,h)=>{const f=Ae(i,h);H.set(i,{resolve:u=>{clearTimeout(f),p(u)},reject:u=>{clearTimeout(f),h(u)},callbacks:s}),Z.postMessage({id:i,type:"validate",buffer:e,configDelta:t},[e])})}const N=["K1","K2","K3","K4","K5","K6","K7"],Ee=[{key:"max_speed_bus_kmh",type:"float",def:120,min:60,max:200},{key:"max_speed_tram_kmh",type:"float",def:100,min:40,max:160},{key:"max_speed_metro_kmh",type:"float",def:150,min:80,max:250},{key:"max_speed_rail_kmh",type:"float",def:300,min:100,max:400},{key:"max_speed_ferry_kmh",type:"float",def:80,min:20,max:150},{key:"max_speed_cablecar_kmh",type:"float",def:30,min:10,max:60},{key:"min_transfer_time_sec",type:"int",def:180,min:30,max:1800},{key:"max_transfer_distance_m",type:"float",def:500,min:50,max:2e3},{key:"max_shape_jump_km",type:"float",def:10,min:1,max:50},{key:"stop_too_close_m",type:"float",def:5,min:1,max:20},{key:"stop_far_from_shape_m",type:"float",def:100,min:20,max:500},{key:"stop_far_from_parent_m",type:"float",def:100,min:10,max:1e3},{key:"feed_expiry_warning_days",type:"int",def:30,min:1,max:60},{key:"service_gap_days",type:"int",def:7,min:3,max:30},{key:"max_trip_duration_hours",type:"float",def:24,min:8,max:72},{key:"min_trip_duration_sec",type:"int",def:60,min:10,max:300},{key:"max_headway_warning_min",type:"int",def:240,min:60,max:720},{key:"bunching_threshold_min",type:"int",def:2,min:1,max:10}];function Fe(e){const t=X(),s=t.result!==null,i=r(s?"upload.load_another":"upload.load");e.innerHTML=`
    <div class="up-grid">

      <div class="up-cell up-files">
        <div class="lp-section-title">${r("upload.files_section")}</div>
        <div id="file-rows" class="lp-rows">
          ${s&&t.result.metrics.file_stats.length>0?Se(t.result.metrics.file_stats):`<div class="lp-placeholder">${r("upload.no_files")}</div>`}
        </div>
      </div>

      <div class="up-cell up-drop">
        <div id="drop-zone" class="drop-zone" role="button" tabindex="0" aria-label="ZIP dosyası yükle">
          <svg class="upload-icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5"/>
          </svg>
          <span class="drop-primary">${r("upload.drag")}</span>
          <label id="upload-btn-label" class="btn btn-primary btn-sm" for="file-input">${z(i)}</label>
          <input type="file" id="file-input" accept=".zip" class="visually-hidden" aria-label="ZIP dosyası seç"/>
        </div>
        <div id="upload-error" class="upload-status hidden"></div>
      </div>

      <div class="up-cell up-stages">
        <div class="lp-section-title">${r("upload.stages_section")}</div>
        <div id="stage-rows" class="lp-rows">
          ${N.map(p=>`
            <div class="lp-stage-row ${s?"done":"waiting"}" data-stage="${p}">
              <span class="lp-icon">${s?"✓":"○"}</span>
              <span class="lp-label">${z(r(`stage.${p}`))}</span>
              <span class="lp-status">${s?"":"—"}</span>
            </div>`).join("")}
        </div>
      </div>

      <div class="up-cell up-score">
        <div id="score-panel" class="score-compact${s?"":" hidden"}">
          ${s?$e(t.result):""}
        </div>
        <button id="btn-report" class="btn btn-primary btn-report" ${s?"":"disabled"}>
          ${r("upload.show_report")}
        </button>
      </div>

    </div>

    <div class="settings-wrap">
      <button class="settings-toggle" id="settings-toggle" type="button">
        <span class="settings-arrow" id="settings-arrow">▶</span>
        ${r("upload.settings_title")}
        ${t.configDelta&&t.configDelta!=="{}"?`<span class="settings-badge">${r("upload.settings_modified")}</span>`:""}
      </button>
      <div class="settings-body hidden" id="settings-body">
        <div class="settings-grid" id="settings-grid">
          ${Ce(qe(t.configDelta))}
        </div>
        <div class="settings-actions">
          <button id="btn-apply-settings" class="btn btn-primary btn-sm">${r("upload.apply")}</button>
          <button id="btn-reset-settings" class="btn btn-ghost btn-sm">${r("upload.reset")}</button>
          <span id="settings-status" class="settings-status hidden"></span>
        </div>
      </div>
    </div>`,He(e)}function Se(e){return e.map(t=>{const s=Math.round(t.bytes/1024),i=r(`file.${t.name}`)!==`file.${t.name}`?r(`file.${t.name}`):r("upload.row_unit");return`
      <div class="lp-file-row done">
        <span class="lp-icon">✓</span>
        <span class="lp-label">${z(t.name)}</span>
        <span class="lp-status">${t.rows.toLocaleString("tr-TR")} ${i} · ${J(s)}</span>
      </div>`}).join("")}function $e(e){const{r5:t}=e.reports,s=[{labelKey:"class.SPEC",weight:"×40%",value:t.spec_score,color:"#2563eb",bg:"#eff6ff"},{labelKey:"class.INTEROP",weight:"×30%",value:t.interop_score,color:"#92400e",bg:"#fef3c7"},{labelKey:"class.QUALITY",weight:"×20%",value:t.quality_score,color:"#15803d",bg:"#f0fdf4"},{labelKey:"class.ANALYTICS",weight:"×10%",value:t.analytics_score,color:"#6d28d9",bg:"#f5f3ff"}],i=t.score>=80?"#15803d":t.score>=60?"#92400e":"#dc2626",p=t.pub_score>=80?"#15803d":t.pub_score>=60?"#d97706":"#dc2626",h=s.map(f=>`<div class="sc-chip" style="background:${f.bg};color:${f.color}">
      <div class="sc-chip-left">
        <span class="sc-chip-label">${r(f.labelKey)}</span>
        <span class="sc-weight">${f.weight}</span>
      </div>
      <span class="sc-chip-val">${f.value.toFixed(1)}</span>
    </div>`).join("");return`
    <div class="sc-dual-header">
      <div class="sc-dual-item">
        <span class="sc-title">${r("score.pub")}</span>
        <span class="sc-overall" style="color:${p}">${t.pub_score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
      <div class="sc-dual-sep"></div>
      <div class="sc-dual-item">
        <span class="sc-title">${r("score.qual")}</span>
        <span class="sc-overall" style="color:${i}">${t.score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
    </div>
    <div class="sc-chips">${h}</div>`}function Ce(e){return Ee.map(t=>{const s=e[t.key]!==void 0?e[t.key]:t.def,i=e[t.key]!==void 0,p=r(`cfg.${t.key}.label`),h=r(`cfg.${t.key}.unit`),f=r(`cfg.${t.key}.desc`);return`
      <div class="sf-row${i?" sf-overridden":""}">
        <label class="sf-label" title="${z(f)}">${z(p)}</label>
        <div class="sf-control">
          <input class="sf-input" type="number" data-key="${t.key}" data-def="${t.def}"
            data-type="${t.type}" min="${t.min}" max="${t.max}"
            step="${t.type==="int"?1:.1}" value="${s}"/>
          <span class="sf-unit">${h}</span>
          <button class="sf-reset btn btn-ghost" data-key="${t.key}" title="${r("upload.reset")}"
            ${i?"":'style="visibility:hidden"'}>✕</button>
        </div>
        <div class="sf-desc">${z(f)} (${r("upload.default_for",{val:t.def})})</div>
      </div>`}).join("")}function qe(e){if(!e||e==="{}")return{};try{const t=JSON.parse(e);if(typeof t!="object"||t===null||Array.isArray(t))return{};const s={};for(const[i,p]of Object.entries(t))typeof p=="number"&&isFinite(p)&&(s[i]=p);return s}catch{return{}}}function He(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),i=e.querySelector("#upload-error");s.addEventListener("change",()=>{const u=s.files?.[0];u&&ne(u,e,i)}),t.addEventListener("dragover",u=>{u.preventDefault(),t.classList.contains("loading")||t.classList.add("drag-over")}),t.addEventListener("dragleave",()=>t.classList.remove("drag-over")),t.addEventListener("drop",u=>{if(u.preventDefault(),t.classList.remove("drag-over"),t.classList.contains("loading"))return;const l=u.dataTransfer?.files[0];l&&ne(l,e,i)}),t.addEventListener("keydown",u=>{(u.key==="Enter"||u.key===" ")&&!t.classList.contains("loading")&&s.click()}),e.querySelector("#btn-report")?.addEventListener("click",()=>{K("domain"),C()});const p=e.querySelector("#settings-toggle"),h=e.querySelector("#settings-body"),f=e.querySelector("#settings-arrow");p.addEventListener("click",()=>{const u=!h.classList.contains("hidden");h.classList.toggle("hidden",u),f.textContent=u?"▶":"▼"}),e.querySelector("#btn-apply-settings")?.addEventListener("click",()=>Ie(e)),e.querySelector("#btn-reset-settings")?.addEventListener("click",()=>Oe(e)),e.querySelectorAll(".sf-reset").forEach(u=>{u.addEventListener("click",()=>{const l=u.dataset.key,a=e.querySelector(`.sf-input[data-key="${CSS.escape(l)}"]`);a&&(a.value=a.dataset.def,a.closest(".sf-row")?.classList.remove("sf-overridden"),u.style.visibility="hidden")})}),e.querySelectorAll(".sf-input").forEach(u=>{u.addEventListener("change",()=>{const l=e.querySelector(`.sf-reset[data-key="${CSS.escape(u.dataset.key)}"]`);l&&(l.style.visibility="visible"),u.closest(".sf-row")?.classList.add("sf-overridden")})})}function Ie(e){const t={};e.querySelectorAll(".sf-input").forEach(p=>{const h=p.dataset.key,f=parseFloat(p.dataset.def),u=p.dataset.type==="int"?parseInt(p.value,10):parseFloat(p.value);!isNaN(u)&&u!==f&&(t[h]=u)});const s=Object.keys(t).length>0?JSON.stringify(t):"";_e(s);const i=e.querySelector("#settings-status");i.textContent=r("upload.settings_saved"),i.className="settings-status ok",setTimeout(()=>{i.className="settings-status hidden"},3e3)}function Oe(e){e.querySelectorAll(".sf-input").forEach(s=>{s.value=s.dataset.def,s.closest(".sf-row")?.classList.remove("sf-overridden");const i=e.querySelector(`.sf-reset[data-key="${CSS.escape(s.dataset.key)}"]`);i&&(i.style.visibility="hidden")}),_e("");const t=e.querySelector("#settings-status");t.textContent=r("upload.settings_reset"),t.className="settings-status ok",setTimeout(()=>{t.className="settings-status hidden"},3e3)}const De=512*1024*1024;async function ne(e,t,s){if(!e.name.endsWith(".zip")){Q(s,{code:"InvalidInput",message:r("upload.error_zip")});return}if(e.size>De){Q(s,{code:"InvalidInput",message:r("upload.error_size",{mb:(e.size/1048576).toFixed(1)})});return}s.className="upload-status hidden",Ge(t,e.name,e.size);try{const i=await e.arrayBuffer(),p=await ze(i,X().configDelta,{onFileList:h=>{const f=t.querySelector("#file-rows");f&&(f.innerHTML="",h.forEach(u=>{const l=document.createElement("div");l.className="lp-file-row reading",l.dataset.file=u.name;const a=Math.round(u.uncompressed_size/1024),o=`${J(a)} ${r("upload.running")}`;l.innerHTML=`
            <span class="lp-icon">○</span>
            <span class="lp-label">${z(u.name)}</span>
            <span class="lp-status">${o}</span>`,f.appendChild(l)}))},onFileDone:(h,f,u)=>{const a=t.querySelector("#file-rows")?.querySelector(`[data-file="${CSS.escape(h)}"]`);if(!a)return;a.className="lp-file-row done",a.querySelector(".lp-icon").textContent="✓";const o=r(`file.${h}`)!==`file.${h}`?r(`file.${h}`):r("upload.row_unit");a.querySelector(".lp-status").textContent=`${f.toLocaleString("tr-TR")} ${o} · ${J(Math.round(u/1024))}`},onStageDone:(h,f)=>{const u=t.querySelector(`[data-stage="${CSS.escape(h)}"]`);if(!u)return;u.className="lp-stage-row done",u.querySelector(".lp-icon").textContent="✓",u.querySelector(".lp-status").textContent=f<1e3?`${f} ms`:`${(f/1e3).toFixed(1)} sn`;const l=N.indexOf(h);if(l>=0&&l<N.length-1){const a=t.querySelector(`[data-stage="${CSS.escape(N[l+1])}"]`);a?.classList.contains("waiting")&&(a.className="lp-stage-row running",a.querySelector(".lp-icon").textContent="⋯",a.querySelector(".lp-status").textContent=r("upload.running"))}}});we(p,e.name),Ke(t,p)}catch(i){xe(t),Q(s,i)}}function Ge(e,t,s){const i=e.querySelector("#drop-zone"),p=e.querySelector("#file-input"),h=e.querySelector("#upload-btn-label"),f=e.querySelector("#score-panel");f&&(f.classList.add("hidden"),f.innerHTML="");const u=e.querySelector("#btn-report");u&&(u.disabled=!0),i.classList.add("loading"),p.disabled=!0,h.setAttribute("aria-disabled","true"),h.style.opacity="0.5",h.style.pointerEvents="none";const l=(s/1048576).toFixed(1),a=i.querySelector(".drop-primary");a&&(a.textContent=`${t} (${l} MB) ${r("upload.running")}`);const o=e.querySelector("#stage-rows");o&&(o.innerHTML=N.map((n,c)=>`
      <div class="lp-stage-row ${c===0?"running":"waiting"}" data-stage="${n}">
        <span class="lp-icon">${c===0?"⋯":"○"}</span>
        <span class="lp-label">${z(r(`stage.${n}`))}</span>
        <span class="lp-status">${r(c===0?"upload.running":"upload.waiting")}</span>
      </div>`).join(""))}function xe(e){const t=e.querySelector("#drop-zone"),s=e.querySelector("#file-input"),i=e.querySelector("#upload-btn-label");t.classList.remove("loading"),s.disabled=!1,i.removeAttribute("aria-disabled"),i.style.opacity="",i.style.pointerEvents=""}function Ke(e,t){xe(e);const s=e.querySelector("#upload-btn-label");s.textContent=r("upload.load_another");const i=e.querySelector("#score-panel");i.className="score-compact",i.innerHTML=$e(t);const p=e.querySelector("#btn-report");p.disabled=!1,p.addEventListener("click",()=>{K("domain"),C()},{once:!0});const h=e.querySelector("#file-rows");h&&t.metrics.file_stats.length>0&&(h.innerHTML=Se(t.metrics.file_stats))}function Q(e,t){const s=ye[t.code]??t.code;e.className="upload-status error",e.innerHTML=`<strong>${z(s)}</strong><p>${z(t.message)}</p>`}function J(e){return e>=1024?`${(e/1024).toFixed(1)} MB`:`${e} KB`}function z(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function Ne(e,t){const{r1:s,r5:i}=t.reports,p=t.metrics,h=Ve(t);e.innerHTML=`
    <div class="report-page">
      ${Be(i,h)}
      ${je(s,t)}
      ${Ue(i)}
      ${Qe(p)}
    </div>`}function Be(e,t){const s=ce(e.pub_score),i=ce(e.score);return`
    <div class="rpt-score-row">
      <div class="rpt-score-card">
        <span class="rpt-score-label">${r("domain.pub_score")}</span>
        <span class="rpt-score-val" style="color:${s}">${e.pub_score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${e.pub_score.toFixed(1)}%;background:${s}"></div></div>
        <span class="rpt-score-hint">${r("domain.pub_hint")}</span>
      </div>
      <div class="rpt-score-card">
        <span class="rpt-score-label">${r("domain.qual_score")}</span>
        <span class="rpt-score-val" style="color:${i}">${e.score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${e.score.toFixed(1)}%;background:${i}"></div></div>
        <span class="rpt-score-hint">${r("domain.qual_hint")}</span>
      </div>
      ${Ye(t)}
    </div>`}function ce(e){return e>=80?"#16a34a":e>=60?"#d97706":"#dc2626"}function je(e,t){const s=new Map(t.notices.map(i=>[i.id,i]));if(!e.publishable){const i=e.blocker_notice_ids.slice(0,5).map(p=>{const h=s.get(p);return h?`<li><code>${G(h.rule_id)}</code> — ${G(h.title||h.message.slice(0,80))}</li>`:""}).join("");return`
      <div class="rpt-r1 rpt-r1-blocked">
        <div class="rpt-r1-icon">✕</div>
        <div class="rpt-r1-body">
          <strong>${r("domain.r1_blocked_title")}</strong>
          <span>${r("domain.r1_blocked_desc",{n:e.blocker_notice_ids.length})}</span>
          ${i?`<ul class="rpt-r1-list">${i}</ul>`:""}
        </div>
      </div>`}if(e.conditional){const i=e.conditional_blocker_notice_ids.slice(0,5).map(p=>{const h=s.get(p);return h?`<li><code>${G(h.rule_id)}</code> — ${G(h.title||h.message.slice(0,80))}</li>`:""}).join("");return`
      <div class="rpt-r1 rpt-r1-conditional">
        <div class="rpt-r1-icon">⚠</div>
        <div class="rpt-r1-body">
          <strong>${r("domain.r1_conditional_title")}</strong>
          <span>${r("domain.r1_conditional_desc",{n:e.conditional_blocker_notice_ids.length})}</span>
          ${i?`<ul class="rpt-r1-list">${i}</ul>`:""}
        </div>
      </div>`}return`
    <div class="rpt-r1 rpt-r1-ok">
      <div class="rpt-r1-icon">✓</div>
      <div class="rpt-r1-body">
        <strong>${r("domain.r1_ok_title")}</strong>
        <span>${r("domain.r1_ok_desc")}</span>
      </div>
    </div>`}function Ue(e){const s=[{labelKey:"class.SPEC",weight:"×40%",value:e.spec_score,color:"#1d4ed8",bg:"#eff6ff"},{labelKey:"class.INTEROP",weight:"×30%",value:e.interop_score,color:"#b45309",bg:"#fef3c7"},{labelKey:"class.QUALITY",weight:"×20%",value:e.quality_score,color:"#15803d",bg:"#f0fdf4"},{labelKey:"class.ANALYTICS",weight:"×10%",value:e.analytics_score,color:"#6d28d9",bg:"#f5f3ff"}].map(i=>`
    <div class="rpt-sub-chip" style="background:${i.bg};color:${i.color}">
      <div class="rpt-sub-top">
        <span class="rpt-sub-label">${r(i.labelKey)}</span>
        <span class="rpt-sub-weight">${i.weight}</span>
      </div>
      <span class="rpt-sub-val">${i.value.toFixed(1)}</span>
    </div>`).join("");return`
    <div>
      <div class="rpt-section-title" style="margin-bottom:.4rem">${r("domain.subscores_title")}</div>
      <div class="rpt-sub-row">${s}</div>
    </div>`}const de=[{key:"CRITICAL",color:"#dc2626"},{key:"HIGH",color:"#d97706"},{key:"MEDIUM",color:"#2563eb"},{key:"LOW",color:"#16a34a"},{key:"INFO",color:"#9ca3af"}];function Ye(e){const t=e.CRITICAL+e.HIGH+e.MEDIUM+e.LOW+e.INFO;if(t===0)return"";const s=15.9155;let i=0;const p=de.filter(f=>e[f.key]>0).map(f=>{const u=e[f.key]/t*100,l=25-i;return i+=u,`<circle cx="18" cy="18" r="${s}" fill="none" stroke="${f.color}" stroke-width="3.5" stroke-dasharray="${u.toFixed(3)} ${(100-u).toFixed(3)}" stroke-dashoffset="${l.toFixed(3)}"/>`}).join(""),h=de.filter(f=>e[f.key]>0).map(f=>{const u=e[f.key],l=(u/t*100).toFixed(1);return`<div class="pie-leg-row"><span class="pie-leg-dot" style="background:${f.color}"></span><span class="pie-leg-label">${r(`domain.sev.${f.key}`)}</span><span class="pie-leg-count">${u.toLocaleString("tr-TR")}</span><span class="pie-leg-pct">${l}%</span></div>`}).join("");return`
    <div class="rpt-score-card rpt-pie-card">
      <span class="rpt-score-label">${r("domain.pie_title")} <span class="rpt-score-total">${t.toLocaleString("tr-TR")}</span></span>
      <div class="rpt-pie-body">
        <svg viewBox="0 0 36 36" class="rpt-pie-svg" aria-hidden="true">
          <circle cx="18" cy="18" r="${s}" fill="none" class="pie-track" stroke-width="3.5"/>
          ${p}
          <text x="18" y="16" text-anchor="middle" dominant-baseline="middle"
            font-size="5.5" font-weight="800" class="pie-center-num">${t.toLocaleString("tr-TR")}</text>
          <text x="18" y="22" text-anchor="middle" dominant-baseline="middle"
            font-size="2.8" class="pie-center-txt">${r("domain.pie_center")}</text>
        </svg>
        <div class="rpt-pie-legend">${h}</div>
      </div>
    </div>`}function Ve(e){const t={CRITICAL:0,HIGH:0,MEDIUM:0,LOW:0,INFO:0};for(const s of e.notices)s.severity in t&&t[s.severity]++;return t}function Qe(e){const s=[{label:r("domain.metric.stops"),value:e.stop_count.toLocaleString("tr-TR")},{label:r("domain.metric.routes"),value:e.route_count.toLocaleString("tr-TR")},{label:r("domain.metric.trips"),value:e.trip_count.toLocaleString("tr-TR")},{label:r("domain.metric.shapes"),value:e.shape_count.toLocaleString("tr-TR")},{label:r("domain.metric.service_days"),value:e.active_service_days.toLocaleString("tr-TR")},{label:r("domain.metric.avg_trips"),value:e.avg_daily_trips.toFixed(0)}].map(p=>`
    <div class="rpt-metric">
      <span class="rpt-metric-val">${p.value}</span>
      <span class="rpt-metric-label">${p.label}</span>
    </div>`).join(""),i=e.file_stats.map(p=>`
    <tr>
      <td><code>${G(p.name)}</code></td>
      <td class="num">${p.rows.toLocaleString("tr-TR")}</td>
      <td class="num">${We(p.bytes)}</td>
    </tr>`).join("");return`
    <div class="card">
      <h3 class="rpt-section-title">${r("domain.metrics_title")}</h3>
      <div class="rpt-metrics-grid">${s}</div>
      ${i?`
        <div class="table-scroll rpt-files">
          <table class="data-table">
            <thead><tr>
              <th>${r("domain.table.file")}</th>
              <th class="num">${r("domain.table.rows")}</th>
              <th class="num">${r("domain.table.size")}</th>
            </tr></thead>
            <tbody>${i}</tbody>
          </table>
        </div>`:""}
    </div>`}function We(e){return e<1024?`${e} B`:e<1024*1024?`${(e/1024).toFixed(1)} KB`:`${(e/1024/1024).toFixed(1)} MB`}function G(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}let w=null,T=null;function Ze(e,t){Xe(),w.querySelector(".mm-title").textContent=e,w.classList.remove("mm-hidden");const s=w.querySelector(".mm-map");T?T.eachLayer(f=>{f instanceof L.TileLayer||T.removeLayer(f)}):(T=L.map(s),L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",{attribution:"© OpenStreetMap katkıcıları",maxZoom:19}).addTo(T));const i=[];if(t.polyline&&t.polyline.length>1){L.polyline(t.polyline,{color:"#f59e0b",weight:3,opacity:.8}).addTo(T);for(const f of t.polyline)i.push(f);if(t.showArrows)for(const f of Je(t.polyline,250))L.marker([f.lat,f.lon],{icon:L.divIcon({html:`<svg width="14" height="14" viewBox="-7 -7 14 14" style="transform:rotate(${f.angle}deg);display:block">
              <polygon points="0,-6 5,4 -5,4" fill="#f59e0b" stroke="#fff" stroke-width="1.5"/>
            </svg>`,className:"",iconSize:[14,14],iconAnchor:[7,7]}),interactive:!1}).addTo(T)}let p=null;if(t.extraPolylines){for(const f of t.extraPolylines)if(f.coords.length>1){L.polyline(f.coords,{color:f.color,weight:f.weight??4,opacity:.95}).addTo(T);for(const u of f.coords)i.push(u);f.zoomTo&&(p=f.coords)}}for(const f of t.pins){const u=f.primary?"#2563eb":"#dc2626",l=f.small?5:9,a=f.small?1.5:2.5;L.circleMarker([f.lat,f.lon],{radius:l,fillColor:u,color:"#fff",weight:a,fillOpacity:.85}).addTo(T).bindPopup(f.label,{maxWidth:260}),i.push([f.lat,f.lon])}p&&p.length>1?T.fitBounds(p,{padding:[60,60],maxZoom:16}):i.length===0?T.setView([39,35],6):i.length===1?T.setView(i[0],16):T.fitBounds(i,{padding:[40,40]});const h=w.querySelector(".mm-legend");t.legendItems&&t.legendItems.length>0?(h.innerHTML=t.legendItems.map(f=>`<div class="mm-legend-item"><div class="mm-dot" style="background:${f.color}"></div>${f.label}</div>`).join(""),h.style.display="flex"):h.style.display="none",setTimeout(()=>T.invalidateSize(),50)}function W(){w?.classList.add("mm-hidden")}function Je(e,t){const i=a=>a*Math.PI/180,p=(a,o,n,c)=>{const d=i(n-a),_=i(c-o),m=Math.sin(d/2)**2+Math.cos(i(a))*Math.cos(i(n))*Math.sin(_/2)**2;return 12742e3*Math.atan2(Math.sqrt(m),Math.sqrt(1-m))},h=(a,o,n,c)=>{const d=i(c-o),_=Math.sin(d)*Math.cos(i(n)),m=Math.cos(i(a))*Math.sin(i(n))-Math.sin(i(a))*Math.cos(i(n))*Math.cos(d);return(Math.atan2(_,m)*180/Math.PI+360)%360},f=[];let u=0,l=t*.5;for(let a=0;a<e.length-1;a++){const[o,n]=e[a],[c,d]=e[a+1],_=p(o,n,c,d),m=h(o,n,c,d);for(;u+_>=l;){const g=(l-u)/_;f.push({lat:o+g*(c-o),lon:n+g*(d-n),angle:m}),l+=t}u+=_}return f}function Xe(){w||(w=document.createElement("div"),w.className="mm-overlay mm-hidden",w.innerHTML=`
    <div class="mm-box" role="dialog" aria-modal="true" aria-label="Harita">
      <div class="mm-header">
        <span class="mm-title"></span>
        <button class="mm-close" title="Kapat" aria-label="Kapat">✕</button>
      </div>
      <div class="mm-map"></div>
      <div class="mm-legend"></div>
    </div>`,document.body.appendChild(w),w.querySelector(".mm-close").addEventListener("click",W),w.addEventListener("click",e=>{e.target===w&&W()}),document.addEventListener("keydown",e=>{e.key==="Escape"&&W()}))}function et(e,t){const s=new Map(t.notices.map(l=>[l.id,l])),i=t.reports.r9.items.reduce((l,a)=>l+a.score_delta,0),p=i>0?(100-t.reports.r5.score)/i:1,h=t.reports.r9.items.reduce((l,a)=>l+a.pub_score_delta,0),f=h>0?(100-t.reports.r5.pub_score)/h:1,u=new Map(t.reports.r9.items.map(l=>[l.rule_id,{qualDelta:l.score_delta*p,pubDelta:l.pub_score_delta*f,count:l.affected_instance_count}]));e.innerHTML=`
    <section class="page-fix">
      ${tt(t.reports.r9.items,s,p,f,t.capped_totals)}
      ${st(t,s,u,t.name_index)}
    </section>`}function tt(e,t,s,i,p){if(e.length===0)return`<div class="card"><h2>${r("fix.r9_title")}</h2><p class="empty">${r("fix.r9_empty")}</p></div>`;const h={};for(const u of t.values())h[u.rule_id]=(h[u.rule_id]??0)+1;const f=e.map((u,l)=>{const a=u.notice_ids[0]?t.get(u.notice_ids[0]):void 0,o=a?.severity??"INFO",n=u.labels.map($=>`<span class="label-badge label-${$}">${r(`label.${$}`)!==`label.${$}`?r(`label.${$}`):$}</span>`).join(""),c=u.pub_score_delta*i,d=u.score_delta*s,_=c>=.05?`<span class="pub-delta">+${c.toFixed(1)}</span>`:'<span class="muted-text">—</span>',m=d>=.05?`<span class="score-delta">+${d.toFixed(1)}</span>`:'<span class="muted-text">—</span>',g=p[u.rule_id],b=g!=null?`<span class="cap-total">${g.toLocaleString("tr-TR")}</span>`:'<span class="muted-text">—</span>',y=h[u.rule_id]??u.affected_instance_count,k=`
      <tr class="r9-main-row" data-idx="${l}">
        <td>
          <span class="r9-arrow">▶</span> <code>${M(u.rule_id)}</code>
          ${a?.title?`<span class="r9-rule-title">${M(a.title)}</span>`:""}
        </td>
        <td style="color:${V[o]}">${x[o]}</td>
        <td>${n}</td>
        <td>${y.toLocaleString("tr-TR")}</td>
        <td>${b}</td>
        <td class="score">${u.priority_score.toFixed(1)}</td>
        <td class="score-delta-cell">${_}</td>
        <td class="score-delta-cell">${m}</td>
        <td>${u.realized_dependent_count}</td>
        <td>${u.fix_effort.toFixed(1)}</td>
      </tr>`,S=`
      <tr class="r9-detail-row" data-for="${l}" hidden>
        <td colspan="10">
          <div class="r9-detail">
            ${a?.title?`<p><strong>${r("fix.r9_message")}</strong> ${M(a.title)}</p>`:""}
            ${a?.remediation?`<p><strong>${r("fix.r9_remediation")}</strong> ${M(a.remediation)}</p>`:""}
          </div>
        </td>
      </tr>`;return k+S}).join("");return`
    <div class="card">
      <h2>${r("fix.r9_title")} <span class="count-badge">${e.length}</span></h2>
      <p class="hint">${r("fix.r9_hint")}</p>
      <div class="table-scroll">
        <table class="data-table" id="r9-table">
          <thead><tr>
            <th>${r("fix.th.rule")}</th>
            <th>${r("fix.th.severity")}</th>
            <th>${r("fix.th.label")}</th>
            <th>${r("fix.th.count")} <span class="col-info" title="${r("fix.th.count.tip")}">ℹ</span></th>
            <th>${r("fix.th.total")} <span class="col-info" title="${r("fix.th.total.tip")}">ℹ</span></th>
            <th class="score">${r("fix.th.score")} <span class="col-info" title="${r("fix.th.score.tip")}">ℹ</span></th>
            <th class="score-delta-cell">${r("fix.th.pub")} <span class="col-info" title="${r("fix.th.pub.tip")}">ℹ</span></th>
            <th class="score-delta-cell">${r("fix.th.quality")} <span class="col-info" title="${r("fix.th.quality.tip")}">ℹ</span></th>
            <th>${r("fix.th.dependent")} <span class="col-info" title="${r("fix.th.dependent.tip")}">ℹ</span></th>
            <th>${r("fix.th.effort")} <span class="col-info" title="${r("fix.th.effort.tip")}">ℹ</span></th>
          </tr></thead>
          <tbody>${f}</tbody>
        </table>
      </div>
    </div>`}function st(e,t,s,i){const p=e.reports.r2.items;if(p.length===0)return`<div class="card"><h2>${r("fix.r2_title")}</h2><p class="empty">${r("fix.r2_empty")}</p></div>`;const f=[...new Map(p.filter(o=>t.has(o.notice_id)).map(o=>{const n=t.get(o.notice_id);return[n.rule_id,n.title||n.rule_id]})).entries()].sort(([o],[n])=>o.localeCompare(n)).map(([o,n])=>`<option value="${M(o)}">${M(o)} — ${M(n)}</option>`).join(""),u=r("fix.filter.all"),l=`
    <div class="filter-bar">
      <label>${r("fix.filter.severity")}
        <select id="sev-filter">
          <option value="">${u}</option>
          <option value="CRITICAL">${x.CRITICAL}</option>
          <option value="HIGH">${x.HIGH}</option>
          <option value="MEDIUM">${x.MEDIUM}</option>
          <option value="LOW">${x.LOW}</option>
          <option value="INFO">${x.INFO}</option>
        </select>
      </label>
      <label>${r("fix.filter.class")}
        <select id="cls-filter">
          <option value="">${u}</option>
          <option value="SPEC">${P.SPEC}</option>
          <option value="INTEROP">${P.INTEROP}</option>
          <option value="QUALITY">${P.QUALITY}</option>
          <option value="ANALYTICS">${P.ANALYTICS}</option>
        </select>
      </label>
      <label>${r("fix.filter.rule")}
        <select id="rule-filter">
          <option value="">${u}</option>
          ${f}
        </select>
      </label>
      <span id="filter-count" class="filter-count"></span>
    </div>`,a=p.map(o=>{const n=t.get(o.notice_id);if(!n)return"";const c=s.get(n.rule_id),d=c!=null&&c.count>0?c.pubDelta/c.count:null,_=c!=null&&c.count>0?c.qualDelta/c.count:null,m=d!=null&&d>=.005?`<span class="pub-delta">+${d.toFixed(2)}</span>`:'<span class="muted-text">—</span>',g=_!=null&&_>=.005?`<span class="score-delta">+${_.toFixed(2)}</span>`:'<span class="muted-text">—</span>',b=r("fix.map_btn"),y=lt(n,i)?`<button class="map-pin-btn" data-notice-id="${M(n.id)}" title="${b}" aria-label="${b}">📍</button>`:"";return`
      <tr data-severity="${n.severity}" data-class="${n.rule_class}" data-rule="${n.rule_id}">
        <td>${M(o.display_label)}</td>
        <td style="color:${V[n.severity]}">${x[n.severity]}</td>
        <td>${P[n.rule_class]}</td>
        <td>${M(n.message)}</td>
        <td>${n.file?M(n.file):"—"}</td>
        <td>${n.line??"—"}</td>
        <td>${n.field?M(n.field):"—"}</td>
        <td class="score-delta-cell">${m}</td>
        <td class="score-delta-cell">${g}</td>
        <td class="map-btn-cell">${y}</td>
      </tr>`}).join("");return`
    <div class="card" id="r2-card">
      <h2>${r("fix.r2_title")} <span class="count-badge">${p.length}</span></h2>
      ${l}
      <div id="r2-cap-warning" class="cap-warning" hidden></div>
      <div class="table-scroll">
        <table class="data-table" id="r2-table">
          <thead><tr>
            <th>${r("fix.r2.th.rule")}</th><th>${r("fix.r2.th.severity")}</th><th>${r("fix.r2.th.class")}</th><th>${r("fix.r2.th.message")}</th>
            <th>${r("fix.r2.th.file")}</th><th>${r("fix.r2.th.row")}</th><th>${r("fix.r2.th.field")}</th>
            <th class="score-delta-cell">${r("fix.r2.th.pub")} <span class="col-info" title="${r("fix.r2.th.pub.tip")}">ℹ</span></th>
            <th class="score-delta-cell">${r("fix.r2.th.quality")} <span class="col-info" title="${r("fix.r2.th.quality.tip")}">ℹ</span></th>
            <th class="map-btn-cell"></th>
          </tr></thead>
          <tbody>${a}</tbody>
        </table>
      </div>
    </div>`}function M(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function v(e,t,s,i){const p=t.stop_coords[e];if(!p)return null;const f=`<strong>${t.stops[e]??e}</strong>${i?` ${i}`:""}<br><code>${e}</code>`;return{lat:p[0],lon:p[1],label:f,primary:s}}const at=new Set(["STP_003","STP_004","STP_005","STP_006","STP_007","STP_008","STP_009","STP_010","STP_011","STP_012","STP_013","STP_014","STP_015","STP_016","STP_017","STP_018","STP_019","STP_020","STP_021","STP_022","STP_023","STP_024","STP_025","STP_026","STP_027","STP_028","STP_029","STP_030","GEO_001","GEO_002","GEO_003","GEO_004","GEO_005","GEO_009","GEO_010","GEO_011","GEO_012","GEO_014","PTH_012","PTH_013","PTH_015","PTH_016","PTH_017","PTH_018","PTH_019","LVL_006","SHP_022","VAT_007"]),ee=new Set(["TRP_001","TRP_002","TRP_003","TRP_004","TRP_005","TRP_006","TRP_007","TRP_008","TRP_009","TRP_010","TRP_011","TRP_012","TRP_013","TRP_014","TRP_015","TRP_016","TRP_017","TRP_018","TRP_019","TRP_020","FRQ_001","FRQ_002","FRQ_003","FRQ_004","FRQ_005","FRQ_006","FRQ_007","FRQ_008","FRQ_009","STM_001","STM_002","STM_003","STM_004","STM_005","STM_006","STM_007","STM_008","STM_009","STM_010","STM_011","STM_013","STM_015","STM_016","STM_018","STM_019","STM_021","STM_022","STM_023","STM_024","STM_027","STM_028","STM_029","STM_030","STM_031","STM_032","STM_034","OPR_006","OPR_017"]),te=new Set(["RTS_001","RTS_002","RTS_003","RTS_004","RTS_005","RTS_006","RTS_007","RTS_008","RTS_009","RTS_010","RTS_011","RTS_012","RTS_013","RTS_014","RTS_015","RTS_016","RTS_017","OPR_001","OPR_002","OPR_003"]);function lt(e,t){const s=e.entity_id??"";if(at.has(e.rule_id))return!!(s&&s in t.stop_coords);if(e.rule_id==="GEO_013")return Object.keys(t.stop_coords).length>0;if(e.rule_id==="SHP_017"){const i=e.message.match(/\(kod: '([^']+)'\)/)?.[1],p=e.details?.ctx_b??"",h=e.details?.ctx_a??"";return!!(i&&i in t.stop_coords)||p.length>0||h.length>0}if(e.rule_id==="STM_014"){const i=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/);return i&&(i[1]in t.stop_coords||i[2]in t.stop_coords)?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_020"){const i=e.details?.stop_a??"",p=e.details?.stop_b??"";return i&&i in t.stop_coords||p&&p in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="STM_025"){const i=e.details?.stop_a??"",p=e.details?.stop_b??"";return i&&i in t.stop_coords||p&&p in t.stop_coords?!0:!!(s&&s in t.trip_shapes)}if(e.rule_id==="OPR_015"){const i=e.details?.shape_id??"";return!!(i&&i in t.shape_coords)}if(e.rule_id==="PTH_014"){const i=e.details?.from_station??"",p=e.details?.to_station??"";return!!(i&&i in t.stop_coords&&p&&p in t.stop_coords)}if(["OPR_007","OPR_008","STM_017"].includes(e.rule_id))return!!(s&&s in t.trip_shapes);if(["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"].includes(e.rule_id))return!!(s&&s in t.shape_coords);if(["STM_012","STM_026","STM_033"].includes(e.rule_id))return!!(s&&s in t.trip_shapes)||!!(s&&s in t.trip_stops);if(e.rule_id==="STM_035"){const i=e.observed_value??"";return!!(i&&i in t.stop_coords)||!!(s&&(s in t.trip_shapes||s in t.trip_stops))}return e.rule_id==="VAT_005"?Object.keys(t.stop_coords).length>0:ee.has(e.rule_id)?!!(s&&(s in t.trip_shapes||s in t.trip_stops)):te.has(e.rule_id)?!!(s&&(t.route_shapes[s]?.length??0)>0):!1}function it(e,t){const s=e.entity_id??"";if(e.rule_id==="SHP_017"){const l=e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",a=e.details?.shape_id??"",o=(e.details?.ctx_b??"").split(",").filter(Boolean),n=(e.details?.ctx_a??"").split(",").filter(Boolean),c=(e.details?.seq_b??"").split(",").filter(Boolean),d=(e.details?.seq_a??"").split(",").filter(Boolean),_=e.observed_value??"",m=[];for(let S=0;S<o.length;S++){const $=c[S]?`#${c[S]} `:"",A=v(o[S],t,!0,`${$}(önceki)`);A&&m.push(A)}const g=_?`#${_} `:"",b=l?v(l,t,!1,`${g}⚠ sıra bozuk`):null;b&&m.push(b);for(let S=0;S<n.length;S++){const $=d[S]?`#${d[S]} `:"",A=v(n[S],t,!0,`${$}(sonraki)`);A&&m.push(A)}const y=a?t.shape_coords[a]??[]:[],k=[];return y.length>1&&k.push({color:"#f59e0b",label:"Güzergah şekli"}),k.push({color:"#2563eb",label:"Komşu duraklar"}),k.push({color:"#dc2626",label:"Sırası bozuk durak"}),{pins:m,polyline:y.length>1?y:void 0,legendItems:k,showArrows:y.length>1}}if(e.rule_id==="STM_014"){const l=e.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/),a=l?v(l[1].trim(),t,!0,"(kalkış)"):null,o=l?v(l[2].trim(),t,!1,"(varış)"):null,n=[a,o].filter(m=>m!==null),c=s?t.trip_shapes[s]??"":"",d=c?t.shape_coords[c]??[]:[],_=[];return d.length>1&&_.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>1&&(_.push({color:"#2563eb",label:"Kalkış durağı"}),_.push({color:"#dc2626",label:"Varış durağı"})),{pins:n,polyline:d.length>1?d:void 0,legendItems:_,showArrows:d.length>1}}if(e.rule_id==="STM_020"){const l=e.details?.stop_a?v(e.details.stop_a,t,!0,"(kalkış)"):null,a=e.details?.stop_b?v(e.details.stop_b,t,!1,"(varış)"):null,o=[l,a].filter(_=>_!==null),n=s?t.trip_shapes[s]??"":"",c=n?t.shape_coords[n]??[]:[],d=[];return c.length>1&&d.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&d.push({color:"#2563eb",label:"Kalkış durağı"}),o.length>1&&d.push({color:"#dc2626",label:"Varış durağı"}),{pins:o,polyline:c.length>1?c:void 0,legendItems:d,showArrows:!0}}if(e.rule_id==="SHP_010"){const l=s?t.shape_coords[s]??[]:[],a=e.observed_value?.match(/^\(([^,]+),([^)]+)\)$/),o=[];if(a){const c=parseFloat(a[1]),d=parseFloat(a[2]);!isNaN(c)&&!isNaN(d)&&o.push({lat:c,lon:d,label:`Tekrarlanan koordinat<br><code>(${a[1]}, ${a[2]})</code>`,primary:!1})}const n=[];return l.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&n.push({color:"#dc2626",label:"Tekrarlanan nokta"}),{pins:o,polyline:l.length>1?l:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_022"){const l=e.message.match(/durağı '([^']+)' güzergah şeklinin/)?.[1]??"",a=v(s,t,!1,"⚠ belirsiz eşleşme"),o=l?t.shape_coords[l]??[]:[],n=a?[a]:[],c=[];return o.length>1&&c.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&c.push({color:"#dc2626",label:"Belirsiz durak"}),{pins:n,polyline:o.length>1?o:void 0,legendItems:c,showArrows:!0}}if(e.rule_id==="SHP_018"||e.rule_id==="SHP_019"){const l=s?t.shape_coords[s]??[]:[],a=l.length>1?[{color:"#f59e0b",label:"Kullanılmayan güzergah şekli"}]:[];return{pins:[],polyline:l.length>1?l:void 0,legendItems:a,showArrows:!0}}if(e.rule_id==="OPR_007"){const l=e.details?.dup_stop??e.message.match(/\(kod: '([^']+)'\)/)?.[1]??"",a=(e.details?.stops??"").split(",").filter(Boolean),o=s?t.trip_shapes[s]??"":"",n=o?t.shape_coords[o]??[]:[],c=new Set,d=[];for(const m of a){if(c.has(m))continue;c.add(m);const g=m===l,b=v(m,t,!g,g?"(tekrar eden)":void 0);b&&(b.small=!g,d.push(b))}const _=[];return n.length>1&&_.push({color:"#f59e0b",label:"Güzergah şekli"}),d.length>1&&_.push({color:"#2563eb",label:"Duraklar"}),l&&_.push({color:"#dc2626",label:"Tekrar eden durak"}),{pins:d,polyline:n.length>1?n:void 0,legendItems:_,showArrows:!0}}if(e.rule_id==="STM_017"){const l=s?t.trip_shapes[s]??"":"",a=l?t.shape_coords[l]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(d=>v(d,t,!0)).filter(d=>d!==null).map(d=>({...d,small:!0})),c=[];return a.length>1&&c.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&c.push({color:"#2563eb",label:"Sefer durakları"}),{pins:n,polyline:a.length>1?a:void 0,legendItems:c,showArrows:!0}}if(e.rule_id==="GEO_006"){const l=s?t.shape_coords[s]??[]:[],a=e.observed_value?.match(/segment\s+(\d+)→(\d+)/),o=[];if(a&&l.length>0){const c=parseInt(a[1],10),d=parseInt(a[2],10),_=l[c],m=l[d];_&&o.push({lat:_[0],lon:_[1],label:`<strong>Atlama başlangıcı</strong><br>Segment ${c}`,primary:!0}),m&&o.push({lat:m[0],lon:m[1],label:`<strong>Atlama bitişi</strong><br>Segment ${d}`,primary:!1})}const n=[];return l.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&(n.push({color:"#dc2626",label:"Atlama başlangıcı"}),n.push({color:"#2563eb",label:"Atlama bitişi"})),{pins:o,polyline:l.length>1?l:void 0,legendItems:n,showArrows:!1}}if(e.rule_id==="SHP_020"){const l=s?t.shape_coords[s]??[]:[],a=e.observed_value?.match(/\(([^,]+),([^)]+)\)$/),o=[];if(a){const c=parseFloat(a[1]),d=parseFloat(a[2]);!isNaN(c)&&!isNaN(d)&&o.push({lat:c,lon:d,label:`Tekrarlayan koordinat<br><code>(${a[1]}, ${a[2]})</code>`,primary:!1})}const n=[];return l.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&n.push({color:"#dc2626",label:"Tekrarlayan nokta"}),{pins:o,polyline:l.length>1?l:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="SHP_009"){const l=s?t.shape_coords[s]??[]:[],a=e.observed_value?.match(/segment (\d+)-(\d+) ∩ (\d+)-(\d+)/),o=[];if(a&&l.length>0){const c=parseInt(a[1],10),d=parseInt(a[2],10),_=parseInt(a[3],10),m=parseInt(a[4],10),g=l[c],b=l[d],y=l[_],k=l[m];g&&o.push({lat:g[0],lon:g[1],label:`Segment ${c}→${d} başlangıcı`,primary:!0}),b&&o.push({lat:b[0],lon:b[1],label:`Segment ${c}→${d} bitişi`,primary:!0}),y&&o.push({lat:y[0],lon:y[1],label:`Segment ${_}→${m} başlangıcı`,primary:!1}),k&&o.push({lat:k[0],lon:k[1],label:`Segment ${_}→${m} bitişi`,primary:!1})}const n=[];return l.length>1&&n.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&(n.push({color:"#2563eb",label:"1. kesişen segment"}),n.push({color:"#dc2626",label:"2. kesişen segment"})),{pins:o,polyline:l.length>1?l:void 0,legendItems:n,showArrows:!0}}if(e.rule_id==="OPR_008"){const l=s?t.trip_shapes[s]??"":"",a=l?t.shape_coords[l]??[]:[],o=[];a.length>1&&o.push({color:"#9ca3af",label:"Güzergah şekli"});const n=[],c=[],d=new Set;for(let _=0;;_++){const m=e.details?.[`bad_seg_${_}_a`],g=e.details?.[`bad_seg_${_}_b`];if(!m||!g)break;const b=t.stop_coords[m],y=t.stop_coords[g];b&&y&&n.push({coords:[[b[0],b[1]],[y[0],y[1]]],color:"#dc2626",weight:5,zoomTo:_===0}),b&&!d.has(m)&&(d.add(m),c.push({lat:b[0],lon:b[1],label:`<code>${m}</code>`,primary:!0,small:!1})),y&&!d.has(g)&&(d.add(g),c.push({lat:y[0],lon:y[1],label:`<code>${g}</code>`,primary:!0,small:!1}))}return n.length>0&&o.push({color:"#dc2626",label:"Bozuk segment"}),o.push({color:"#2563eb",label:"Segment uçları"}),{pins:c,polyline:a.length>1?a.map(_=>[_[0],_[1]]):void 0,extraPolylines:n,legendItems:o,showArrows:!1}}if(e.rule_id==="STM_025"){const l=s?t.trip_shapes[s]??"":"",a=l?t.shape_coords[l]??[]:[],o=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",c=e.details?.stop_b??"",d=[];for(const b of o){if(b===n||b===c)continue;const y=v(b,t,!0);y&&d.push({...y,small:!0})}const _=n?v(n,t,!0,"(kalkış)"):null;_&&d.push(_);const m=c?v(c,t,!1,"(varış — çok kısa süre)"):null;m&&d.push(m);const g=[];return a.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),g.push({color:"#dc2626",label:"Kısa süreli segment sonu"}),{pins:d,polyline:a.length>1?a:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="OPR_015"){const l=e.details?.shape_id??"",a=l?t.shape_coords[l]??[]:[],o=a.length>1?[{color:"#f59e0b",label:"Tek güzergah şekli"}]:[];return{pins:[],polyline:a.length>1?a:void 0,legendItems:o,showArrows:!0}}function i(l){const a=t.shape_trips[l]??"";return(a?t.trip_stops[a]??[]:[]).map(n=>v(n,t,!0)).filter(n=>n!==null).map(n=>({...n,small:!0}))}if(e.rule_id==="SHP_007"){const l=s?t.shape_coords[s]??[]:[],a=i(s),o=[];return l.length>1&&o.push({color:"#f59e0b",label:"Güzergah şekli"}),a.length>0&&o.push({color:"#2563eb",label:"Sefer durakları"}),{pins:a,polyline:l.length>1?l:void 0,legendItems:o,showArrows:l.length>1}}if(e.rule_id==="SHP_012"){const l=s?t.shape_coords[s]??[]:[],a=i(s),o=[];return l.length>1&&o.push({color:"#f59e0b",label:"Güzergah şekli"}),a.length>0&&o.push({color:"#2563eb",label:"Sefer durakları"}),{pins:a,polyline:l.length>1?l:void 0,legendItems:o,showArrows:!0}}if(e.rule_id==="SHP_014"){const l=s?t.shape_coords[s]??[]:[],a=e.details?.problem_stop??"",o=e.details?.trip_id??"",n=o?t.trip_stops[o]??[]:[],c=[];for(const g of n){if(g===a)continue;const b=v(g,t,!0);b&&c.push({...b,small:!0})}const d=e.details?.endpoint==="start"?"⚠ ilk durak — shape başından uzak":"⚠ son durak — shape bitişinden uzak",_=a?v(a,t,!1,d):null;_&&c.push(_);const m=[];return l.length>1&&m.push({color:"#f59e0b",label:"Güzergah şekli"}),n.length>0&&m.push({color:"#2563eb",label:"Sefer durakları"}),_&&m.push({color:"#dc2626",label:"Uçtan uzak durak"}),{pins:c,polyline:l.length>1?l:void 0,legendItems:m,showArrows:!0}}if(e.rule_id==="SHP_015"){const l=s?t.shape_coords[s]??[]:[],a=i(s),o=[];return l.length>1&&o.push({color:"#f59e0b",label:"Güzergah şekli"}),a.length>0&&o.push({color:"#2563eb",label:"Sefer durakları"}),{pins:a,polyline:l.length>1?l:void 0,legendItems:o,showArrows:!0}}if(e.rule_id==="SHP_016"){const l=s?t.shape_coords[s]??[]:[],a=e.details?.first_stop??"",o=e.details?.trip_id??"",n=o?t.trip_stops[o]??[]:[],c=[];for(const m of n){if(m===a)continue;const g=v(m,t,!0);g&&c.push({...g,small:!0})}const d=a?v(a,t,!1,"⚠ ilk durak — shape'in yanlış ucuna düşüyor"):null;d&&c.push(d);const _=[];return l.length>1&&_.push({color:"#f59e0b",label:"Güzergah şekli (ters)"}),n.length>0&&_.push({color:"#2563eb",label:"Sefer durakları"}),d&&_.push({color:"#dc2626",label:"İlk durak (yanlış uçta)"}),{pins:c,polyline:l.length>1?l:void 0,legendItems:_,showArrows:!0}}if(e.rule_id==="STM_012"){const l=s?t.trip_shapes[s]??"":"",a=l?t.shape_coords[l]??[]:[],o=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",c=e.details?.stop_b??"",d=[];for(const b of o){if(b===n||b===c)continue;const y=v(b,t,!0);y&&d.push({...y,small:!0})}const _=n?v(n,t,!0,"(kalkış — imkansız hız)"):null,m=c?v(c,t,!1,"(varış — imkansız hız)"):null;_&&d.push(_),m&&d.push(m);const g=[];return a.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),g.push({color:"#dc2626",label:"İmkansız hız segmenti"}),{pins:d,polyline:a.length>1?a:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="STM_026"){const l=s?t.trip_shapes[s]??"":"",a=l?t.shape_coords[l]??[]:[],o=s?t.trip_stops[s]??[]:[],n=e.details?.stop_a??"",c=e.details?.stop_b??"",d=[];for(const b of o){if(b===n||b===c)continue;const y=v(b,t,!0);y&&d.push({...y,small:!0})}const _=n?v(n,t,!0,"(kalkış)"):null,m=c?v(c,t,!1,"(varış — çok uzak)"):null;_&&d.push(_),m&&d.push(m);const g=[];return a.length>1&&g.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&g.push({color:"#2563eb",label:"Sefer durakları"}),g.push({color:"#dc2626",label:"Aşırı uzak segment sonu"}),{pins:d,polyline:a.length>1?a:void 0,legendItems:g,showArrows:!0}}if(e.rule_id==="STM_033"){const l=s?t.trip_shapes[s]??"":"",a=l?t.shape_coords[l]??[]:[],n=(s?t.trip_stops[s]??[]:[]).map(d=>v(d,t,!1,"(tek durak — sefer kullanılamaz)")).filter(d=>d!==null),c=[];return a.length>1&&c.push({color:"#f59e0b",label:"Güzergah şekli"}),c.push({color:"#dc2626",label:"Tek durak (kullanılamaz sefer)"}),{pins:n,polyline:a.length>1?a:void 0,legendItems:c,showArrows:a.length>1}}if(e.rule_id==="STM_035"){const l=s?t.trip_shapes[s]??"":"",a=l?t.shape_coords[l]??[]:[],o=s?t.trip_stops[s]??[]:[],n=e.observed_value??"",c=[];for(const m of o){if(m===n)continue;const g=v(m,t,!0);g&&c.push({...g,small:!0})}const d=n?v(n,t,!1,"(ardışık iki kez ziyaret)"):null;d&&c.push(d);const _=[];return a.length>1&&_.push({color:"#f59e0b",label:"Güzergah şekli"}),o.length>0&&_.push({color:"#2563eb",label:"Sefer durakları"}),d&&_.push({color:"#dc2626",label:"Tekrar ziyaret edilen durak"}),{pins:c,polyline:a.length>1?a:void 0,legendItems:_,showArrows:a.length>1}}if(e.rule_id==="PTH_014"){const l=e.details?.from_station??"",a=e.details?.to_station??"",o=t.stop_coords[l],n=t.stop_coords[a];if(!o||!n)return defaultMapOptions(e,t);const c=t.stops[l]??l,d=t.stops[a]??a;return{center:[(o[0]+n[0])/2,(o[1]+n[1])/2],zoom:15,markers:[{lat:o[0],lon:o[1],color:"#ef4444",label:c,title:`İstasyon: ${c}`},{lat:n[0],lon:n[1],color:"#3b82f6",label:d,title:`İstasyon: ${d}`}],polylines:[{coords:[[o[0],o[1]],[n[0],n[1]]],color:"#ef4444",dashArray:"6,4",weight:2}],showArrows:!1}}if(e.rule_id==="VAT_005"){const l=new Set((e.details?.isolated_stops??"").split(",").filter(Boolean)),a=new Set;for(const n of Object.values(t.trip_stops))for(const c of n)a.add(c);const o=[];for(const[n,c]of Object.entries(t.stop_coords)){const d=l.has(n),_=a.has(n);if(!d&&!_)continue;const m=t.stops[n]??n;o.push({lat:c[0],lon:c[1],label:d?`<strong>⚠ İzole</strong><br><strong>${m}</strong><br><code>${n}</code>`:`<strong>${m}</strong><br><code>${n}</code>`,primary:!d,small:!d})}return{pins:o,legendItems:[{color:"#2563eb",label:"Bağlı duraklar"},{color:"#dc2626",label:"İzole duraklar"}]}}if(e.rule_id==="VAT_007"){const l=(e.details?.routes??"").split(",").filter(Boolean),a=["#3b82f6","#10b981","#8b5cf6","#f97316","#06b6d4"],o=t.stop_coords[s],n=t.stops[s]??s,c=o?[{lat:o[0],lon:o[1],label:`<strong>Terminal: ${n}</strong><br><code>${s}</code>`,primary:!1}]:[],d=[],_=[];c.length>0&&_.push({color:"#dc2626",label:"Terminal durağı"});let m=0;for(const g of l.slice(0,5)){const b=t.route_shapes[g]??[],y=a[m%a.length];let k=!1;for(const S of b.slice(0,2)){const $=t.shape_coords[S]??[];$.length>1&&(d.push({coords:$,color:y,weight:3,zoomTo:!1}),k=!0)}if(k){const S=t.routes[g]??g;_.push({color:y,label:S}),m++}}return{pins:c,extraPolylines:d,legendItems:_,showArrows:!1}}if(e.rule_id==="GEO_013"){const l=Object.entries(t.stop_coords).map(([a,o])=>({lat:o[0],lon:o[1],label:t.stops[a]?`<strong>${t.stops[a]}</strong><br><code>${a}</code>`:`<code>${a}</code>`,primary:!0,small:!0}));return{pins:l,legendItems:[{color:"#2563eb",label:`${l.length} durak`}]}}if(ee.has(e.rule_id)){const l=t.trip_shapes[s]??"",a=l?t.shape_coords[l]??[]:[],o=t.trip_stops[s]??[],n=t.trip_routes[s]??"",c=n?t.routes[n]??"":"",d=o.flatMap((m,g)=>{const b=v(m,t,!0,`#${g+1}`);return b?[{...b,small:!0}]:[]}),_=[];return a.length>1&&_.push({color:"#f59e0b",label:c?`Güzergah şekli — ${c}`:"Güzergah şekli"}),d.length>0&&_.push({color:"#2563eb",label:"Sefer durakları"}),{pins:d,polyline:a.length>1?a:void 0,legendItems:_,showArrows:a.length>1}}if(te.has(e.rule_id)){const l=t.route_shapes[s]??[],a=t.routes[s]??s,o=[];for(const n of l.slice(0,4)){const c=t.shape_coords[n]??[];c.length>1&&o.push({coords:c,color:"#f59e0b",weight:3,zoomTo:o.length===0})}return o.length===0?{pins:[]}:{pins:[],extraPolylines:o,legendItems:[{color:"#f59e0b",label:`Güzergah şekilleri — ${a}`}],showArrows:!1}}const p=t.stop_coords[s];if(!p)return{pins:[]};const[h,f]=p,u=t.stops[s]??s;if(e.rule_id==="STP_029"){const l=e.message.match(/üst istasyonundan \(([^)]+)\)/)?.[1]??"",a=[{lat:h,lon:f,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],o=l?v(l,t,!1,"(üst istasyon)"):null;o&&a.push(o);const n=a.length>1?[{color:"#2563eb",label:"Durak"},{color:"#dc2626",label:"Üst İstasyon"}]:[];return{pins:a,legendItems:n}}if(e.rule_id==="STP_016"){const l=e.observed_value?.match(/== '([^']+)'/)?.[1]??"",a=l?t.stops[l]??l:"",o=a?`<strong>${u}</strong> = <strong>${a}</strong><br><code>${s}</code> ve <code>${l}</code>`:`<strong>${u}</strong><br><code>${s}</code>`;return{pins:[{lat:h,lon:f,label:o,primary:!0}]}}if(e.rule_id==="STP_017"){const l=e.observed_value?.match(/to '([^']+)'/)?.[1]??"",a=[{lat:h,lon:f,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],o=l?v(l,t,!1):null;o&&a.push(o);const n=a.length>1?[{color:"#2563eb",label:"Durak 1"},{color:"#dc2626",label:"Durak 2"}]:[];return{pins:a,legendItems:n}}if(e.rule_id==="GEO_009"){const l=e.observed_value?.match(/\(shape '([^']+)'\)/)?.[1]??"",a=l?t.shape_coords[l]??[]:[],o=[{color:"#2563eb",label:"Durak"}];return a.length>0&&o.push({color:"#f59e0b",label:"Güzergah şekli"}),{pins:[{lat:h,lon:f,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}],polyline:a.length>1?a:void 0,legendItems:o}}return{pins:[{lat:h,lon:f,label:`<strong>${u}</strong><br><code>${s}</code>`,primary:!0}]}}function rt(e,t,s){if(e.querySelectorAll(".r9-main-row").forEach(c=>{c.addEventListener("click",()=>{const d=c.dataset.idx,_=e.querySelector(`.r9-detail-row[data-for="${d}"]`);if(!_)return;_.hidden=!_.hidden;const m=c.querySelector(".r9-arrow");m&&(m.textContent=_.hidden?"▶":"▼")})}),t){const c=new Map(t.notices.map(d=>[d.id,d]));e.querySelectorAll(".map-pin-btn").forEach(d=>{d.addEventListener("click",_=>{_.stopPropagation();const m=d.dataset.noticeId??"",g=c.get(m);if(!g)return;const b=it(g,t.name_index);if(b.pins.length===0&&!b.polyline)return;const y=new Set(["SHP_007","SHP_009","SHP_010","SHP_012","SHP_014","SHP_015","SHP_016","SHP_018","SHP_019","SHP_020","GEO_006","GEO_007"]),k=g.entity_id??"";let S;if(y.has(g.rule_id))S=`${r("fix.map.shape")}: ${k}`;else if(ee.has(g.rule_id)){const $=t.name_index.trip_routes[k]??"",A=$?t.name_index.routes[$]??"":"",E=t.name_index.trips[k]??"";S=A?`${A}${E?` — ${E}`:""}`:E||k}else te.has(g.rule_id)?S=t.name_index.routes[k]??k:S=t.name_index.stops[k]??k;Ze(`${g.rule_id} — ${S}`,b)})})}const i=e.querySelector("#sev-filter"),p=e.querySelector("#cls-filter"),h=e.querySelector("#rule-filter"),f=e.querySelector("#r2-table");if(!i||!p||!f)return;const u=h?Array.from(h.options).filter(c=>c.value!=="").map(c=>[c.value,c.text]):[],l=[["CRITICAL",x.CRITICAL],["HIGH",x.HIGH],["MEDIUM",x.MEDIUM],["LOW",x.LOW],["INFO",x.INFO]],a=[["SPEC",P.SPEC],["INTEROP",P.INTEROP],["QUALITY",P.QUALITY],["ANALYTICS",P.ANALYTICS]];function o(c,d,_,m){c.innerHTML=`<option value="">${r("fix.filter.all")}</option>`+m.filter(([g])=>_.has(g)).map(([g,b])=>`<option value="${g}"${g===d?" selected":""}>${b}</option>`).join(""),d&&!_.has(d)&&(c.value="")}function n(){const c=i.value,d=p.value,_=h?.value??"";let m=0,g=0;const b=new Set,y=new Set,k=new Set;f.querySelectorAll("tbody tr").forEach(R=>{g++;const B=R.dataset.severity??"",ae=R.dataset.class??"",le=R.dataset.rule??"",j=!c||B===c,U=!d||ae===d,Y=!_||le===_;R.style.display=j&&U&&Y?"":"none",j&&U&&Y&&m++,U&&Y&&b.add(B),j&&Y&&y.add(ae),j&&U&&k.add(le)});const S=c||d||_,$=e.querySelector("#filter-count");$&&($.textContent=S?r("fix.filter.count",{visible:m,total:g}):"");const A=e.querySelector("#r2-card h2 .count-badge");A&&(A.textContent=String(S?m:g));const E=e.querySelector("#r2-cap-warning");if(E){const R=_&&s?s[_]:void 0;R!=null?(E.hidden=!1,E.textContent=`⚠ ${r("fix.cap_warning",{shown:m,total:R})}`):E.hidden=!0}o(i,c,b,l),o(p,d,y,a),h&&(h.innerHTML=`<option value="">${r("fix.filter.all")}</option>`+u.filter(([R])=>k.has(R)).map(([R,B])=>`<option value="${R}"${R===_?" selected":""}>${M(B)}</option>`).join(""),_&&!k.has(_)&&(h.value=""))}i.addEventListener("change",n),p.addEventListener("change",n),h?.addEventListener("change",n)}const I={CRITICAL:0,HIGH:1,MEDIUM:2,LOW:3,INFO:4};function ot(e,t){const s=nt(t.notices),i=t.notices.length;e.innerHTML=`
    <div class="rules-page">
      <div class="rules-summary-bar">
        <span class="rules-summary-text">${r("rules.summary",{total:i,rules:s.length})}</span>
      </div>
      <div class="rules-accordion">
        ${s.map(p=>ct(p)).join("")}
      </div>
    </div>`,dt(e)}function nt(e){const t=new Map;for(const s of e){t.has(s.rule_id)||t.set(s.rule_id,{rule_id:s.rule_id,max_severity:s.severity,rule_class:s.rule_class,notices:[]});const i=t.get(s.rule_id);i.notices.push(s),(I[s.severity]??9)<(I[i.max_severity]??9)&&(i.max_severity=s.severity)}return Array.from(t.values()).sort((s,i)=>{const p=(I[s.max_severity]??9)-(I[i.max_severity]??9);return p!==0?p:i.notices.length-s.notices.length})}function ct(e){const t=V[e.max_severity],s=x[e.max_severity],i=P[e.rule_class]??e.rule_class,p=e.rule_class.toLowerCase(),f=[...e.notices].sort((l,a)=>(I[l.severity]??9)-(I[a.severity]??9)).map(l=>{const a=pt(l.message),o=a.length>160?a.slice(0,160)+"…":a,n=V[l.severity]??"#666",c=r(`entity.${l.entity_type}`)!==`entity.${l.entity_type}`?r(`entity.${l.entity_type}`):l.entity_type,d=l.entity_id?`<span class="muted-text">${F(c)}</span> <code class="entity-id">${F(l.entity_id)}</code>`:`<span class="muted-text">${F(c)}</span>`;return`
      <tr>
        <td style="color:${n};white-space:nowrap">${x[l.severity]}</td>
        <td>${d}</td>
        <td class="msg-cell">${F(o)}</td>
      </tr>`}).join(""),u=e.notices[0]?.remediation??"";return`
    <div class="rule-acc-section">
      <button class="rule-acc-header" type="button" aria-expanded="false">
        <span class="acc-arrow">▶</span>
        <span class="rule-title-block">
          <code class="rule-code rule-code-lg">${F(e.rule_id)}</code>
          ${u?`<span class="rule-desc-text">${F(u)}</span>`:""}
        </span>
        <span class="badge badge-${p}">${F(i)}</span>
        <span class="rule-sev-badge" style="color:${t}">${F(s)}</span>
        <span class="acc-count rule-notice-count">${e.notices.length}</span>
      </button>
      <div class="rule-acc-body" hidden>
        <div class="table-scroll">
          <table class="data-table">
            <thead><tr>
              <th>${r("rules.th.severity")}</th>
              <th>${r("rules.th.entity")}</th>
              <th>${r("rules.th.message")}</th>
            </tr></thead>
            <tbody>${f}</tbody>
          </table>
        </div>
      </div>
    </div>`}function dt(e){e.querySelectorAll(".rule-acc-header").forEach(t=>{t.addEventListener("click",()=>{const i=t.closest(".rule-acc-section").querySelector(".rule-acc-body"),p=t.querySelector(".acc-arrow"),h=!i.hidden;i.hidden=h,p.textContent=h?"▶":"▼",t.setAttribute("aria-expanded",String(!h))})})}function pt(e){return e.replace(/\btrip_id\b/g,"sefer").replace(/\bstop_sequence\b/g,"Durak Sırası").replace(/\barrival_time\b/g,"Varış zamanı").replace(/\bdeparture_time\b/g,"Kalkış zamanı").replace(/\bheadway\b/g,"Sefer Aralığı")}function F(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function ut(e,t,s){e.innerHTML=`
    <section class="page-export-wide">
      <div class="card">
        <h2>${r("export.title")}</h2>
        <p class="export-filename">${r("export.source")} <code>${q(s)}</code></p>
        <div class="export-actions">
          <button id="btn-export-html" class="btn btn-primary">${r("export.html")}</button>
          <button id="btn-export-csv"  class="btn btn-secondary">${r("export.csv")}</button>
          <button id="btn-export-json" class="btn btn-secondary">${r("export.json")}</button>
          <button id="btn-export-pdf"  class="btn btn-secondary">${r("export.pdf")}</button>
        </div>
      </div>
    </section>`,e.querySelector("#btn-export-html").addEventListener("click",()=>_t(t,s)),e.querySelector("#btn-export-csv").addEventListener("click",()=>ft(t,s)),e.querySelector("#btn-export-json").addEventListener("click",()=>ht(t,s)),e.querySelector("#btn-export-pdf").addEventListener("click",()=>mt(t,s))}function pe(e){const t=e==null?"":String(e),s=/^[=+\-@]/.test(t)?`'${t}`:t;return s.includes(",")||s.includes('"')||s.includes(`
`)?`"${s.replace(/"/g,'""')}"`:s}function ft(e,t){const s=[r("export.csv.rule"),r("export.csv.severity"),r("export.csv.class"),r("export.csv.message"),r("export.csv.entity_id"),r("export.csv.file"),r("export.csv.row")],i=e.notices.map(u=>[u.rule_id,x[u.severity]??u.severity,P[u.rule_class]??u.rule_class,u.message,u.entity_id??"",u.file??"",u.line!=null?String(u.line):""].map(pe).join(",")),h="\uFEFF"+[s.map(pe).join(","),...i].join(`\r
`),f=new Blob([h],{type:"text/csv; charset=utf-8"});se(f,t.replace(/\.zip$/i,"-report.csv"))}function ht(e,t){const s=new Blob([JSON.stringify(e,null,2)],{type:"application/json"});se(s,t.replace(/\.zip$/i,"-report.json"))}function Te(e,t){const{r1:s,r5:i}=e.reports,p=s.publishable?s.conditional?r("export.html.publishable_cond"):r("export.html.publishable_ok"):r("export.html.publishable_blocked"),h=e.notices.map(u=>`
    <tr>
      <td>${q(u.rule_id)}</td>
      <td>${x[u.severity]}</td>
      <td>${P[u.rule_class]}</td>
      <td>${q(u.message)}</td>
      <td>${u.entity_id?q(u.entity_id):""}</td>
      <td>${u.file?q(u.file):""}</td>
      <td>${u.line??""}</td>
    </tr>`).join(""),f=r("export.html.breakdown",{spec:i.spec_score.toFixed(1),interop:i.interop_score.toFixed(1),quality:i.quality_score.toFixed(1),analytics:i.analytics_score.toFixed(1)});return`<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8"/>
  <title>${r("export.html.doc_title")} - ${q(t)}</title>
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
  <h1>${r("export.html.h1")}</h1>
  <p style="color:#64748b;font-size:.9rem">${r("export.html.source")} ${q(t)}</p>

  <div class="summary">
    <div class="kpi">
      <span class="kpi-value">${p}</span>
      <span class="kpi-label">${r("export.html.publishability")}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${i.score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">${r("export.html.qual_score")}</span>
      <span class="score-breakdown">${f}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${i.pub_score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">${r("export.html.pub_score")}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${e.notices.length}</span>
      <span class="kpi-label">${r("export.html.total")}</span>
    </div>
  </div>

  <h2>${r("export.html.h2_findings")}</h2>
  <table>
    <thead><tr>
      <th>${r("export.html.th.rule")}</th>
      <th>${r("export.html.th.severity")}</th>
      <th>${r("export.html.th.class")}</th>
      <th>${r("export.html.th.message")}</th>
      <th>${r("export.html.th.entity_id")}</th>
      <th>${r("export.html.th.file")}</th>
      <th>${r("export.html.th.row")}</th>
    </tr></thead>
    <tbody>${h}</tbody>
  </table>
</body>
</html>`}function _t(e,t){const s=Te(e,t),i=new Blob([s],{type:"text/html; charset=utf-8"});se(i,t.replace(/\.zip$/i,"-report.html"))}function mt(e,t){const s=Te(e,t),i=window.open("","_blank");if(!i){alert(r("export.popup_blocked"));return}i.document.write(s),i.document.close(),i.focus(),i.print()}function se(e,t){const s=URL.createObjectURL(e),i=document.createElement("a");i.href=s,i.download=t,i.click(),URL.revokeObjectURL(s)}function q(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}function gt(){localStorage.getItem("gtfs-theme")==="dark"&&document.documentElement.classList.add("dark")}function ue(){const e=document.documentElement.classList.toggle("dark");localStorage.setItem("gtfs-theme",e?"dark":"light"),bt()}function bt(){const e=document.documentElement.classList.contains("dark");document.querySelectorAll(".dark-toggle").forEach(t=>{t.textContent=e?"☀":"☾",t.title=r(e?"theme.light":"theme.dark")})}function fe(){const e=document.documentElement.classList.contains("dark");return`<button class="btn btn-ghost dark-toggle" title="${r(e?"theme.light":"theme.dark")}">${e?"☀":"☾"}</button>`}function he(){return`<button class="btn btn-ghost lang-toggle" title="Switch language">${r("lang.switch")}</button>`}function C(){const e=X(),t=document.getElementById("app");if(e.page==="upload"||!e.result){t.innerHTML=`
      <header class="app-header">
        <h1>${r("header.title")}</h1>
        <span style="flex:1"></span>
        ${he()}
        ${fe()}
      </header>
      <main id="page-root"></main>`,t.querySelector(".dark-toggle").addEventListener("click",ue),t.querySelector(".lang-toggle").addEventListener("click",()=>{re(ie()==="tr"?"en":"tr"),C()}),Fe(document.getElementById("page-root"));return}const s=["domain","fix","rules","export"],i={domain:"nav.report",fix:"nav.fix",rules:"nav.rules",export:"nav.export"},p=s.map(f=>`
      <button class="nav-btn ${e.page===f?"active":""}" data-page="${f}">
        ${r(i[f])}
      </button>`).join("");t.innerHTML=`
    <header class="app-header">
      <h1>${r("header.title")}</h1>
      <span class="header-filename">${yt(e.fileName)}</span>
      <button id="btn-back" class="btn btn-ghost">${r("header.back")}</button>
      <button id="btn-new" class="btn btn-secondary">${r("header.new")}</button>
      ${he()}
      ${fe()}
    </header>
    <nav class="app-nav">${p}</nav>
    <main id="page-root"></main>`,t.querySelector("#btn-back").addEventListener("click",()=>{K("upload"),C()}),t.querySelector("#btn-new").addEventListener("click",()=>{K("upload"),C()}),t.querySelector(".dark-toggle").addEventListener("click",ue),t.querySelector(".lang-toggle").addEventListener("click",()=>{re(ie()==="tr"?"en":"tr"),C()}),t.querySelectorAll(".nav-btn").forEach(f=>{f.addEventListener("click",()=>{K(f.dataset.page),C()})});const h=document.getElementById("page-root");switch(e.page){case"domain":Ne(h,e.result);break;case"fix":et(h,e.result),rt(h,e.result,e.result.capped_totals);break;case"rules":ot(h,e.result);break;case"export":ut(h,e.result,e.fileName);break}}function yt(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}gt();C();
