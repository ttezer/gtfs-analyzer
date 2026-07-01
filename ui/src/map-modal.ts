/* WP-18a — Harita popup (durak / koordinat hataları) */

import * as L from 'leaflet';
import 'leaflet/dist/leaflet.css';

export interface MapPin {
  lat: number;
  lon: number;
  label: string;
  primary: boolean; // mavi = birincil, kırmızı = referans
  small?: boolean;  // küçük marker (çok sayıda pin olduğunda)
  color?: string;   // özel renk (verilirse primary mavi/kırmızı yerine bu kullanılır)
}

let overlay: HTMLElement | null = null;
let leafletMap: L.Map | null = null;

export interface MapOptions {
  pins: MapPin[];
  polyline?: [number, number][];  // shape çizgisi (ana renk)
  extraPolylines?: Array<{        // ek renkli segmentler (bozuk segment vurgusu gibi)
    coords: [number, number][];
    color: string;
    weight?: number;
    zoomTo?: boolean;             // bu segmente zoom yap
  }>;
  legendItems?: Array<{ color: string; label: string }>;
  showArrows?: boolean;           // polyline üzerine yön okları (her ~250m)
}

export function openMapModal(title: string, opts: MapOptions): void {
  ensureOverlay();
  overlay!.querySelector<HTMLElement>('.mm-title')!.textContent = title;
  overlay!.classList.remove('mm-hidden');

  const mapDiv = overlay!.querySelector<HTMLElement>('.mm-map')!;

  if (!leafletMap) {
    leafletMap = L.map(mapDiv);
    L.tileLayer('https://mt0.google.com/vt/lyrs=p&x={x}&y={y}&z={z}', {
      attribution: '© Google',
      maxZoom: 19,
    }).addTo(leafletMap);
  } else {
    leafletMap.eachLayer((layer: L.Layer) => {
      if (!(layer instanceof L.TileLayer)) leafletMap!.removeLayer(layer);
    });
  }

  const bounds: [number, number][] = [];

  // Shape polyline (arka planda, pinlerden önce)
  if (opts.polyline && opts.polyline.length > 1) {
    L.polyline(opts.polyline, { color: '#f59e0b', weight: 3, opacity: 0.8 }).addTo(leafletMap);
    for (const pt of opts.polyline) bounds.push(pt);

    // Yön okları — her ~250m'de bir küçük üçgen
    if (opts.showArrows) {
      for (const arrow of arrowsAlongPolyline(opts.polyline, 250)) {
        L.marker([arrow.lat, arrow.lon], {
          icon: L.divIcon({
            html: `<svg width="14" height="14" viewBox="-7 -7 14 14" style="transform:rotate(${arrow.angle}deg);display:block">
              <polygon points="0,-6 5,4 -5,4" fill="#f59e0b" stroke="#fff" stroke-width="1.5"/>
            </svg>`,
            className: '',
            iconSize: [14, 14],
            iconAnchor: [7, 7],
          }),
          interactive: false,
        }).addTo(leafletMap);
      }
    }
  }

  // Ek renkli segmentler (bozuk hız segmentleri vb.)
  let zoomTarget: [number, number][] | null = null;
  if (opts.extraPolylines) {
    for (const seg of opts.extraPolylines) {
      if (seg.coords.length > 1) {
        L.polyline(seg.coords, { color: seg.color, weight: seg.weight ?? 4, opacity: 0.95 }).addTo(leafletMap);
        for (const pt of seg.coords) bounds.push(pt);
        if (seg.zoomTo) zoomTarget = seg.coords;
      }
    }
  }

  // Pinler
  for (const pin of opts.pins) {
    const color = pin.color ?? (pin.primary ? '#2563eb' : '#dc2626');
    const radius = pin.small ? 5 : 9;
    const weight = pin.small ? 1.5 : 2.5;
    L.circleMarker([pin.lat, pin.lon], {
      radius,
      fillColor: color,
      color: '#fff',
      weight,
      fillOpacity: 0.85,
    })
      .addTo(leafletMap)
      .bindPopup(pin.label, { maxWidth: 260 });
    bounds.push([pin.lat, pin.lon]);
  }

  if (zoomTarget && zoomTarget.length > 1) {
    // Bozuk segmente zoom yap
    leafletMap.fitBounds(zoomTarget, { padding: [60, 60], maxZoom: 16 });
  } else if (bounds.length === 0) {
    // Hiçbir şey yok — Türkiye merkezi
    leafletMap.setView([39.0, 35.0], 6);
  } else if (bounds.length === 1) {
    leafletMap.setView(bounds[0], 16);
  } else {
    leafletMap.fitBounds(bounds, { padding: [40, 40] });
  }

  // Legend
  const legend = overlay!.querySelector<HTMLElement>('.mm-legend')!;
  if (opts.legendItems && opts.legendItems.length > 0) {
    legend.innerHTML = opts.legendItems.map(it =>
      `<div class="mm-legend-item"><div class="mm-dot" style="background:${it.color}"></div>${it.label}</div>`
    ).join('');
    legend.style.display = 'flex';
  } else {
    legend.style.display = 'none';
  }

  // Leaflet görünür hale geldikten sonra boyutu yeniden hesapla
  setTimeout(() => leafletMap!.invalidateSize(), 50);
}

export function closeMapModal(): void {
  overlay?.classList.add('mm-hidden');
}

function arrowsAlongPolyline(
  pts: [number, number][],
  intervalM: number,
): Array<{ lat: number; lon: number; angle: number }> {
  const R = 6371000;
  const toRad = (d: number) => (d * Math.PI) / 180;
  const haversineM = (la1: number, lo1: number, la2: number, lo2: number) => {
    const dLat = toRad(la2 - la1), dLon = toRad(lo2 - lo1);
    const a = Math.sin(dLat / 2) ** 2 + Math.cos(toRad(la1)) * Math.cos(toRad(la2)) * Math.sin(dLon / 2) ** 2;
    return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
  };
  const bearingDeg = (la1: number, lo1: number, la2: number, lo2: number) => {
    const dLon = toRad(lo2 - lo1);
    const y = Math.sin(dLon) * Math.cos(toRad(la2));
    const x = Math.cos(toRad(la1)) * Math.sin(toRad(la2)) - Math.sin(toRad(la1)) * Math.cos(toRad(la2)) * Math.cos(dLon);
    return ((Math.atan2(y, x) * 180) / Math.PI + 360) % 360;
  };

  const arrows: Array<{ lat: number; lon: number; angle: number }> = [];
  let cum = 0;
  let next = intervalM * 0.5; // ilk ok yarım aralıkta başlasın
  for (let i = 0; i < pts.length - 1; i++) {
    const [la1, lo1] = pts[i], [la2, lo2] = pts[i + 1];
    const seg = haversineM(la1, lo1, la2, lo2);
    const angle = bearingDeg(la1, lo1, la2, lo2);
    while (cum + seg >= next) {
      const t = (next - cum) / seg;
      arrows.push({ lat: la1 + t * (la2 - la1), lon: lo1 + t * (lo2 - lo1), angle });
      next += intervalM;
    }
    cum += seg;
  }
  return arrows;
}

function ensureOverlay(): void {
  if (overlay) return;
  overlay = document.createElement('div');
  overlay.className = 'mm-overlay mm-hidden';
  overlay.innerHTML = `
    <div class="mm-box" role="dialog" aria-modal="true" aria-label="Harita">
      <div class="mm-header">
        <span class="mm-title"></span>
        <button class="mm-close" title="Kapat" aria-label="Kapat">✕</button>
      </div>
      <div class="mm-map"></div>
      <div class="mm-legend"></div>
    </div>`;
  document.body.appendChild(overlay);

  overlay.querySelector('.mm-close')!.addEventListener('click', closeMapModal);
  // Overlay dışına tıklayınca kapat
  overlay.addEventListener('click', e => { if (e.target === overlay) closeMapModal(); });
  // Escape tuşu
  document.addEventListener('keydown', e => { if (e.key === 'Escape') closeMapModal(); });
}
