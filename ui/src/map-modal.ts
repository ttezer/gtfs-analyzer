/* WP-18a — Map popup (stop / coordinate errors) */

import * as L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import { t } from './i18n';

export interface MapPin {
  lat: number;
  lon: number;
  label: string;
  primary: boolean; // blue = primary, red = reference
  small?: boolean;  // small marker (when there are many pins)
  color?: string;   // custom color (replaces primary blue/red when provided)
  stroke?: string;  // inline map: ring color (white by default)
}

let overlay: HTMLElement | null = null;
let leafletMap: L.Map | null = null;

export const MAX_MAP_PINS = 5_000;
const MAX_MAP_POLYLINE_POINTS = 100_000;
const MAX_MAP_EXTRA_POLYLINES = 100;
const MAX_MAP_EXTRA_POINTS = 10_000;
const MAX_MAP_ARROWS = 2_000;

export interface MapOptions {
  pins: MapPin[];
  polyline?: [number, number][];  // shape line (base color)
  polylineColor?: string;         // base line color (orange by default; black when a segment is highlighted)
  extraPolylines?: Array<{        // extra colored segments (such as a faulty-segment highlight)
    coords: [number, number][];
    color: string;
    weight?: number;
    zoomTo?: boolean;             // zoom to this segment
    arrows?: boolean;             // inline map: route-direction arrows
  }>;
  /** Dashed circles around a place the finding is about (radius in metres), e.g. two stops at one
   *  point whose faulty step is too short to see as a line. The inline map zooms to them. */
  highlights?: Array<{ center: [number, number]; radiusMeters: number; color: string }>;
  legendItems?: Array<{ color: string; label: string }>;
  showArrows?: boolean;           // direction arrows over the polyline (every ~250 m)
  focus?: [number, number];       // inline map: focus this point (such as the first/last stop)
  focusZoom?: number;             // inline map: rule-specific scale around the focused point
}

/**
 * Map base layer: the standard OpenStreetMap layer with attribution. Only tiles for the
 * visible region are requested; feed data is sent to no server.
 */
function addBaseLayer(map: L.Map): void {
  L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    // The page runs with COEP require-corp (WASM threads). OSM tiles do not send a CORP
    // header but allow CORS; if a tile is not fetched with a CORS request, the browser blocks it.
    crossOrigin: 'anonymous',
    attribution: '&copy; <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap</a>',
  }).addTo(map);
}

/**
 * Mounts the same Leaflet drawing inside a card.
 * It shares the MapOptions contract with the modal; the caller must run the returned
 * cleanup function when closing.
 */
export function mountInlineMap(container: HTMLElement, opts: MapOptions): () => void {
  container.innerHTML = '';
  const map = L.map(container, { zoomControl: true, attributionControl: true, preferCanvas: true });
  addBaseLayer(map);
  const bounds: [number, number][] = [];
  const polyline = capPoints(opts.polyline, MAX_MAP_POLYLINE_POINTS);

  if (polyline && polyline.length > 1) {
    L.polyline(polyline, {
      color: opts.polylineColor ?? '#64748b',
      weight: opts.polylineColor ? 5 : 3,
      opacity: 0.95,
    }).addTo(map);
    bounds.push(...polyline);
  }

  let zoomTarget: [number, number][] | null = null;
  for (const seg of opts.extraPolylines?.slice(0, MAX_MAP_EXTRA_POLYLINES) ?? []) {
    const coords = capPoints(seg.coords, MAX_MAP_EXTRA_POINTS);
    if (!coords || coords.length < 2) continue;
    L.polyline(coords, { color: seg.color, weight: seg.weight ?? 4, opacity: 0.95 }).addTo(map);
    bounds.push(...coords);
    if (seg.zoomTo) zoomTarget = coords;
    if (seg.arrows) {
      // Direction arrows: at most ~20 arrows along the line, in the route color.
      const total = coords.slice(1).reduce((sum, p, i) => sum + Math.hypot(p[0] - coords[i][0], p[1] - coords[i][1]) * 111_000, 0);
      for (const arrow of arrowsAlongPolyline(coords, Math.max(150, total / 20)).slice(0, 40)) {
        L.marker([arrow.lat, arrow.lon], {
          icon: L.divIcon({
            html: `<svg width="14" height="14" viewBox="-7 -7 14 14" style="transform:rotate(${arrow.angle}deg);display:block"><polygon points="0,-6 5,4 -5,4" fill="${seg.color}" stroke="#fff" stroke-width="1.5"/></svg>`,
            className: '', iconSize: [14, 14], iconAnchor: [7, 7],
          }),
          interactive: false,
        }).addTo(map);
      }
    }
  }

  let highlightBounds: L.LatLngBounds | null = null;
  for (const h of opts.highlights?.slice(0, 5) ?? []) {
    const circle = L.circle(h.center, {
      radius: h.radiusMeters, color: h.color, weight: 3, dashArray: '6 5', fillColor: h.color, fillOpacity: 0.08, interactive: false,
    }).addTo(map);
    highlightBounds = highlightBounds ? highlightBounds.extend(circle.getBounds()) : circle.getBounds();
    bounds.push(h.center);
  }

  let primaryLabels = 0;
  const ordered = [...opts.pins.slice(0, MAX_MAP_PINS)].sort((a, b) => Number(a.primary) - Number(b.primary));
  for (const pin of ordered) {
    // On the card map, primary marks the subject's location (red); the others are context (gray).
    const color = pin.color ?? (pin.primary ? '#dc2626' : '#64748b');
    L.circleMarker([pin.lat, pin.lon], {
      radius: pin.small ? 4 : 9,
      fillColor: color,
      color: pin.stroke ?? '#fff',
      weight: 2,
      fillOpacity: 0.9,
    }).addTo(map).bindPopup(pin.label, { maxWidth: 260 });
    // The marked stop gets a permanent label: the route color may also be red, so the point alone is ambiguous.
    if (pin.primary && primaryLabels++ < 5) {
      L.tooltip({ permanent: true, direction: 'top', offset: [0, -8], className: 'planner-map-label' })
        .setLatLng([pin.lat, pin.lon]).setContent(pin.label).addTo(map);
    }
    bounds.push([pin.lat, pin.lon]);
  }

  const target = zoomTarget;
  const applyView = () => {
    if (opts.focus) map.setView(opts.focus, opts.focusZoom ?? 15);
    else if (highlightBounds) map.fitBounds(highlightBounds.pad(1.5), { maxZoom: 17 });
    else if (target && target.length > 1) map.fitBounds(target, { padding: [24, 24], maxZoom: 16 });
    else if (bounds.length > 1) map.fitBounds(bounds, { padding: [24, 24] });
    else if (bounds.length === 1) map.setView(bounds[0], 14);
    else map.setView([39, 35], 5);
  };
  applyView();

  // The box may not have its final size while the card opens; Leaflet calculates tiles and
  // the view using the current size (one tile, labels trapped in a corner). Measure again once it settles.
  let sized = false;
  const observer = typeof ResizeObserver !== 'undefined'
    ? new ResizeObserver(() => {
      map.invalidateSize();
      if (!sized && container.clientWidth > 0 && container.clientHeight > 0) { sized = true; applyView(); }
    })
    : null;
  observer?.observe(container);
  setTimeout(() => { map.invalidateSize(); applyView(); }, 0);
  return () => { observer?.disconnect(); map.remove(); };
}

export function openMapModal(title: string, opts: MapOptions): void {
  ensureOverlay();
  overlay!.querySelector<HTMLElement>('.mm-title')!.textContent = title;
  overlay!.classList.remove('mm-hidden');

  const mapDiv = overlay!.querySelector<HTMLElement>('.mm-map')!;

  if (!leafletMap) {
    leafletMap = L.map(mapDiv);
    addBaseLayer(leafletMap);
  } else {
    leafletMap.eachLayer((layer: L.Layer) => {
      if (!(layer instanceof L.TileLayer)) leafletMap!.removeLayer(layer);
    });
  }

  const bounds: [number, number][] = [];
  const polyline = capPoints(opts.polyline, MAX_MAP_POLYLINE_POINTS);

  // Shape polyline (in the background, before pins)
  if (polyline && polyline.length > 1) {
    // The highlighted (black) base line is slightly thicker and fully opaque: in the gray
    // version it disappeared into the map base.
    const isBase = opts.polylineColor !== undefined;
    L.polyline(polyline, {
      color: opts.polylineColor ?? '#f59e0b',
      weight: isBase ? 4 : 3,
      opacity: isBase ? 0.95 : 0.8,
    }).addTo(leafletMap);
    for (const pt of polyline) bounds.push(pt);

    // Direction arrows — a small triangle every ~250 m
    if (opts.showArrows) {
      for (const arrow of arrowsAlongPolyline(polyline, 250).slice(0, MAX_MAP_ARROWS)) {
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

  // Extra colored segments (faulty speed segments, etc.)
  let zoomTarget: [number, number][] | null = null;
  if (opts.extraPolylines) {
    for (const seg of opts.extraPolylines.slice(0, MAX_MAP_EXTRA_POLYLINES)) {
      const coords = capPoints(seg.coords, MAX_MAP_EXTRA_POINTS);
      if (coords && coords.length > 1) {
        L.polyline(coords, { color: seg.color, weight: seg.weight ?? 4, opacity: 0.95 }).addTo(leafletMap);
        for (const pt of coords) bounds.push(pt);
        if (seg.zoomTo) zoomTarget = coords;
      }
    }
  }

  // Pins
  for (const pin of opts.pins.slice(0, MAX_MAP_PINS)) {
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
    // Zoom to the faulty segment.
    leafletMap.fitBounds(zoomTarget, { padding: [60, 60], maxZoom: 16 });
  } else if (bounds.length === 0) {
    // Nothing to show — center on Turkey.
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

  // Recalculate the size after Leaflet becomes visible.
  setTimeout(() => leafletMap!.invalidateSize(), 50);
}

function capPoints(points: [number, number][] | undefined, max: number): [number, number][] | undefined {
  if (!points || points.length <= max) return points;
  const out: [number, number][] = [points[0]];
  const step = (points.length - 1) / (max - 1);
  for (let i = 1; i < max - 1; i++) out.push(points[Math.round(i * step)]);
  out.push(points[points.length - 1]);
  return out;
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
  let next = intervalM * 0.5; // start the first arrow halfway through the interval
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
    <div class="mm-box" role="dialog" aria-modal="true" aria-label="${t('fix.map.title')}">
      <div class="mm-header">
        <span class="mm-title"></span>
        <button class="mm-close" title="${t('fix.map.close')}" aria-label="${t('fix.map.close')}">✕</button>
      </div>
      <div class="mm-map"></div>
      <div class="mm-legend"></div>
    </div>`;
  document.body.appendChild(overlay);

  overlay.querySelector('.mm-close')!.addEventListener('click', closeMapModal);
  // Close when clicking outside the overlay.
  overlay.addEventListener('click', e => { if (e.target === overlay) closeMapModal(); });
  // Escape key.
  document.addEventListener('keydown', e => { if (e.key === 'Escape') closeMapModal(); });
}
