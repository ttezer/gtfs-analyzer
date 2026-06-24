import cytoscape, { type Core, type ElementDefinition } from 'cytoscape';
import {
  Accessibility,
  ArrowLeftRight,
  BadgeCheck,
  Building2,
  BusFront,
  CalendarCheck2,
  CalendarDays,
  ClipboardCheck,
  Clock3,
  FileText,
  FileWarning,
  Group,
  Info,
  Languages,
  Layers,
  Map as MapIcon,
  MapPin,
  MapPinned,
  Network,
  Repeat2,
  Route,
  Shapes,
  Ticket,
  WalletCards,
  Waypoints,
  type IconNode,
} from 'lucide';
import type { Notice, Severity, ValidationResult } from '../types';
import { SEVERITY_COLOR, SEVERITY_TR, t, tMsg } from '../i18n';
import { setFixFileFilter, setPage } from '../state';
import { escHtml } from '../escape';
import { compareSeverityThenCount } from '../severity-order';
import {
  buildFileSummaries,
  formatFileBytes,
  type FileSummary,
} from '../file-summary';
import {
  CORE_FILE_IDS,
  CORE_FILE_ID_SET,
  FILE_MAP_EDGES,
  FILE_MAP_GROUPS,
  FILE_MAP_NODES,
  getBaseVisibleFileIds,
  getVisibleFileIdsForSelection,
  type FileMapEdge,
} from '../file-map-schema';

let activeGraph: Core | null = null;
let activeResizeObserver: ResizeObserver | null = null;
let activeThemeHandler: (() => void) | null = null;

const SEVERITIES: readonly Severity[] = [
  'CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO',
];

// Cytoscape draws nodes onto a canvas/SVG data-URI and cannot resolve CSS
// custom properties, so the graph needs concrete colors. Derive them from the
// same --color-* variables index.html defines (single source of truth; see
// also SEVERITY_COLOR used for DOM panels). Fall back to the known palette if
// the stylesheet is somehow unavailable. file-map.ts is lazy-loaded after the
// inline stylesheet is parsed, so getComputedStyle is reliable here.
function cssColor(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  return value || fallback;
}

const NODE_COLORS: Record<Severity, string> = {
  CRITICAL: cssColor('--color-critical', '#dc2626'),
  HIGH: cssColor('--color-high', '#ea580c'),
  MEDIUM: cssColor('--color-medium', '#ca8a04'),
  LOW: cssColor('--color-low', '#16a34a'),
  INFO: cssColor('--color-info', '#2563eb'),
};

const FILE_NODE_WIDTH = 172;
const FILE_NODE_HEIGHT = 64;
const GROUP_HORIZONTAL_GAP = 28;
const GROUP_VERTICAL_GAP = 30;
const LAYOUT_MAX_WIDTH = 1260;

const GROUP_LAYOUT_ORDER = [
  'external',
  'calendar',
  'support',
  'core',
  'geometry',
  'station',
  'flex',
  'network',
  'fare-v1',
  'fare-v2',
] as const;

export function renderFileMap(root: HTMLElement, result: ValidationResult): void {
  activeGraph?.destroy();
  activeResizeObserver?.disconnect();
  if (activeThemeHandler) {
    window.removeEventListener('gtfs-theme-change', activeThemeHandler);
  }
  activeGraph = null;
  activeResizeObserver = null;
  activeThemeHandler = null;

  const summaries = buildFileSummaries(result, {
    includeAllSpec: true,
    includeGeneral: false,
  });
  const summaryByName = new Map(summaries.map((summary) => [summary.name, summary]));
  const baseVisibleIds = getBaseVisibleFileIds(
    summaries.map((summary) => ({
      id: summary.name,
      hasFindings: summary.notices.length > 0,
      unknown: summary.unknown,
    })),
  );
  const dynamicFileIds = summaries
    .filter((summary) => summary.unknown)
    .map((summary) => summary.name);
  const allGraphFileIds = [
    ...FILE_MAP_NODES.map((file) => file.id),
    ...dynamicFileIds,
  ];

  root.innerHTML = `
    <section class="file-map-page">
      <div class="file-map-toolbar">
        <div class="file-map-heading">
          <h2>${t('fileMap.title')}</h2>
          <span>${t('fileMap.subtitle')}</span>
        </div>
        <div class="file-map-filters">
          <label>
            <span>${t('fileMap.fileFilter')}</span>
            <select id="file-map-presence">
              <option value="all">${t('fileMap.allFiles')}</option>
              <option value="present">${t('fileMap.presentFiles')}</option>
            </select>
          </label>
          <label>
            <span>${t('fileMap.severityFilter')}</span>
            <select id="file-map-severity">
              <option value="all">${t('fileMap.allSeverities')}</option>
              ${SEVERITIES.map((severity) => `
                <option value="${severity}">${SEVERITY_TR[severity]}</option>
              `).join('')}
            </select>
          </label>
          <label>
            <span>${t('fileMap.selectFile')}</span>
            <select id="file-map-jump">
              <option value="">${t('fileMap.coreView')}</option>
              ${CORE_FILE_IDS.map((fileId) => `
                <option value="${fileId}">${fileId}</option>
              `).join('')}
            </select>
          </label>
          <div class="file-map-zoom" aria-label="${t('fileMap.zoomControls')}">
            <button type="button" id="file-map-zoom-out" title="${t('fileMap.zoomOut')}">&minus;</button>
            <button type="button" id="file-map-fit" title="${t('fileMap.fit')}">&#9635;</button>
            <button type="button" id="file-map-zoom-in" title="${t('fileMap.zoomIn')}">+</button>
          </div>
        </div>
      </div>
      <div class="file-map-layout">
        <div class="file-map-canvas-wrap">
          <div id="file-map-canvas" role="application" aria-label="${t('fileMap.canvasLabel')}"></div>
          <div class="file-map-legend">
            ${SEVERITIES.map((severity) => `
              <span><i style="background:${NODE_COLORS[severity]}"></i>${SEVERITY_TR[severity]}</span>
            `).join('')}
            <span><i class="clean"></i>${t('fileMap.clean')}</span>
            <span><i class="missing"></i>${t('fileMap.missing')}</span>
          </div>
        </div>
        <aside id="file-map-panel" class="file-map-panel" aria-live="polite"></aside>
      </div>
    </section>`;

  const canvas = root.querySelector<HTMLElement>('#file-map-canvas');
  const panel = root.querySelector<HTMLElement>('#file-map-panel');
  if (!canvas || !panel) return;

  activeGraph = cytoscape({
    container: canvas,
    elements: buildElements(summaryByName),
    layout: { name: 'preset', fit: false },
    minZoom: 0.35,
    maxZoom: 2.2,
    wheelSensitivity: 0.18,
    boxSelectionEnabled: false,
    autoungrabify: true,
    style: graphStyles(),
  });

  const graph = activeGraph;
  activeThemeHandler = () => {
    refreshGraphTheme(graph);
    layoutVisibleGraph(graph);
  };
  window.addEventListener('gtfs-theme-change', activeThemeHandler);
  activeResizeObserver = new ResizeObserver(() => {
    graph.resize();
    layoutVisibleGraph(graph);
  });
  activeResizeObserver.observe(canvas);

  let selectedFileId: string | null = null;
  let explorationNodeIds = new Set<string>(baseVisibleIds);

  const updateJumpOptions = (): void => {
    const jump = root.querySelector<HTMLSelectElement>('#file-map-jump');
    if (!jump) return;
    const currentValue = selectedFileId ?? '';
    jump.innerHTML = `
      <option value="">${t('fileMap.coreView')}</option>
      ${allGraphFileIds
        .filter((fileId) => explorationNodeIds.has(fileId))
        .map((fileId) => `<option value="${fileId}">${fileId}</option>`)
        .join('')}`;
    jump.value = currentValue;
  };

  const updateGraphView = (fit = true): void => {
    const presence = root.querySelector<HTMLSelectElement>('#file-map-presence')?.value ?? 'all';
    const severity = root.querySelector<HTMLSelectElement>('#file-map-severity')?.value ?? 'all';

    graph.nodes('[kind = "file"]').forEach((mapNode) => {
      const isPresent = mapNode.data('present') === 'true';
      const severityCount = Number(mapNode.data(`count_${severity}`) ?? 0);
      const visibleByExploration = explorationNodeIds.has(mapNode.id());
      const visibleByPresence = presence === 'all' || isPresent;
      const isCoreContext = CORE_FILE_ID_SET.has(mapNode.id());
      const isSelected = mapNode.id() === selectedFileId;
      const matchesSeverity = severity === 'all' || severityCount > 0;
      const visibleBySeverity = matchesSeverity || isCoreContext || isSelected;
      mapNode.toggleClass(
        'map-filter-muted',
        severity !== 'all' && severityCount === 0 && (isCoreContext || isSelected),
      );
      mapNode.style(
        'display',
        visibleByExploration && visibleByPresence && visibleBySeverity
          ? 'element'
          : 'none',
      );
    });

    graph.edges().forEach((edge) => {
      const belongsToSelection = selectedFileId !== null
        && (edge.source().id() === selectedFileId || edge.target().id() === selectedFileId);
      const endpointsVisible = edge.source().style('display') !== 'none'
        && edge.target().style('display') !== 'none';
      edge.style('display', belongsToSelection && endpointsVisible ? 'element' : 'none');
    });

    graph.nodes('[kind = "group"]').forEach((group) => {
      const hasVisibleChild = group.children().some(
        (child) => child.style('display') !== 'none',
      );
      group.style('display', hasVisibleChild ? 'element' : 'none');
    });
    if (fit) layoutVisibleGraph(graph);
  };

  const resetToCoreView = (): void => {
    selectedFileId = null;
    explorationNodeIds = new Set<string>(baseVisibleIds);
    graph.elements().removeClass('map-selected map-related');
    const presence = root.querySelector<HTMLSelectElement>('#file-map-presence');
    const severity = root.querySelector<HTMLSelectElement>('#file-map-severity');
    if (presence) presence.value = 'all';
    if (severity) severity.value = 'all';
    updateJumpOptions();
    renderOverviewPanel(panel, summaryByName);
    updateGraphView();
  };

  const selectFile = (fileName: string): void => {
    selectedFileId = fileName;
    explorationNodeIds = getVisibleFileIdsForSelection(
      fileName,
      baseVisibleIds,
      (candidateId) => (summaryByName.get(candidateId)?.notices.length ?? 0) > 0,
    );
    graph.elements().removeClass('map-selected map-related');
    const selected = graph.getElementById(fileName);
    selected.addClass('map-selected');
    selected.connectedEdges().connectedNodes().filter(
      (node) => explorationNodeIds.has(node.id()),
    ).addClass('map-related');
    const summary = summaryByName.get(fileName);
    const schemaNode = FILE_MAP_NODES.find((item) => item.id === fileName);
    if (summary) {
      renderFilePanel(panel, summary, schemaNode?.specUrl, resetToCoreView);
    }
    updateJumpOptions();
    updateGraphView();
  };

  const selectEdge = (edge: FileMapEdge): void => {
    graph.elements().removeClass('map-selected map-related');
    graph.getElementById(edge.id).addClass('map-selected');
    graph.getElementById(edge.source).addClass('map-related');
    graph.getElementById(edge.target).addClass('map-related');
    renderEdgePanel(panel, edge, resetToCoreView);
  };

  graph.on('tap', 'node[kind = "file"]', (event) => {
    selectFile(event.target.id());
  });
  graph.on('tap', 'edge', (event) => {
    const schemaEdge = FILE_MAP_EDGES.find((edge) => edge.id === event.target.id());
    if (schemaEdge) selectEdge(schemaEdge);
  });

  root.querySelector('#file-map-presence')?.addEventListener('change', () => updateGraphView());
  root.querySelector('#file-map-severity')?.addEventListener('change', () => updateGraphView());
  root.querySelector<HTMLSelectElement>('#file-map-jump')?.addEventListener('change', (event) => {
    const fileName = (event.currentTarget as HTMLSelectElement).value;
    if (fileName) selectFile(fileName);
    else resetToCoreView();
  });
  root.querySelector('#file-map-zoom-in')?.addEventListener('click', () => {
    graph.zoom({ level: Math.min(graph.zoom() * 1.2, graph.maxZoom()), renderedPosition: viewportCenter(canvas) });
  });
  root.querySelector('#file-map-zoom-out')?.addEventListener('click', () => {
    graph.zoom({ level: Math.max(graph.zoom() / 1.2, graph.minZoom()), renderedPosition: viewportCenter(canvas) });
  });
  root.querySelector('#file-map-fit')?.addEventListener('click', () => {
    layoutVisibleGraph(graph);
  });

  resetToCoreView();
}

function buildElements(
  summaryByName: Map<string, FileSummary>,
): ElementDefinition[] {
  const groups = Object.entries(FILE_MAP_GROUPS).map(([id, label]) => ({
    data: {
      id: `group-${id}`,
      kind: 'group',
      label: t(`fileMap.group.${id}`) === `fileMap.group.${id}`
        ? label
        : t(`fileMap.group.${id}`),
    },
  }));

  const nodes = FILE_MAP_NODES.map((schemaNode) =>
    buildFileNodeElement(
      schemaNode.id,
      schemaNode.group,
      schemaNode.x,
      schemaNode.y,
      summaryByName.get(schemaNode.id),
    ));

  const dynamicNodes = [...summaryByName.values()]
    .filter((summary) => summary.unknown)
    .map((summary, index) =>
      buildFileNodeElement(
        summary.name,
        'external',
        850 + (index % 3) * 210,
        980 + Math.floor(index / 3) * 110,
        summary,
      ));

  const edges = FILE_MAP_EDGES.map((edge) => ({
    data: {
      id: edge.id,
      source: edge.source,
      target: edge.target,
      label: edge.field,
    },
  }));
  return [...groups, ...nodes, ...dynamicNodes, ...edges];
}

function buildFileNodeElement(
  fileId: string,
  group: string,
  x: number,
  y: number,
  summary: FileSummary | undefined,
): ElementDefinition {
    const rows = summary?.info?.rows ?? 0;
    const noticeCount = summary?.notices.length ?? 0;
    const status = summary?.missing
      ? 'missing'
      : noticeCount > 0
        ? (summary?.worstSeverity ?? 'INFO').toLowerCase()
        : 'clean';
    return {
      data: {
        id: fileId,
        kind: 'file',
        group,
        parent: `group-${group}`,
        label: summary?.unknown
          ? `${fileId}\n${t('fileMap.nonSpec')} | ${noticeCount.toLocaleString()} ${t('fileMap.findings')}`
          : `${fileId}\n${rows.toLocaleString()} ${t('fileMap.rows')} | ${noticeCount.toLocaleString()} ${t('fileMap.findings')}`,
        present: summary?.info || summary?.unknown ? 'true' : 'false',
        status,
        count_CRITICAL: summary?.severityCounts.CRITICAL ?? 0,
        count_HIGH: summary?.severityCounts.HIGH ?? 0,
        count_MEDIUM: summary?.severityCounts.MEDIUM ?? 0,
        count_LOW: summary?.severityCounts.LOW ?? 0,
        count_INFO: summary?.severityCounts.INFO ?? 0,
        decoration: fileNodeDecoration(
          fileId,
          group,
          summary?.unknown ?? false,
          status,
          document.documentElement.classList.contains('dark'),
        ),
      },
      position: { x, y },
      classes: `${status}${summary?.unknown ? ' spec-external' : ''}`,
    };
}

function graphStyles(): cytoscape.StylesheetJson {
  const dark = document.documentElement.classList.contains('dark');
  return [
    {
      selector: 'node[kind = "group"]',
      style: {
        'background-color': dark ? '#182233' : '#f8fafc',
        'background-opacity': dark ? 0.58 : 0.82,
        'border-color': dark ? '#334155' : '#cbd5e1',
        'border-width': 1,
        'border-style': 'solid',
        'label': 'data(label)',
        'font-size': 13,
        'font-weight': 700,
        'color': dark ? '#94a3b8' : '#475569',
        'text-valign': 'top',
        'text-halign': 'center',
        'text-background-color': dark ? '#182233' : '#f8fafc',
        'text-background-opacity': 1,
        'text-background-padding': '3px',
        'padding': '22px',
        'shape': 'round-rectangle',
      },
    },
    {
      selector: '#group-calendar',
      style: {
        'background-color': dark ? '#151d36' : '#f5f7ff',
        'border-color': dark ? '#6366f1' : '#818cf8',
        'color': dark ? '#a5b4fc' : '#4f46e5',
        'text-background-color': dark ? '#151d36' : '#f5f7ff',
        'border-width': 1.5,
      },
    },
    {
      selector: '#group-core',
      style: {
        'background-color': dark ? '#102827' : '#f0fdfa',
        'border-color': dark ? '#14b8a6' : '#2dd4bf',
        'color': dark ? '#5eead4' : '#0f766e',
        'text-background-color': dark ? '#102827' : '#f0fdfa',
        'border-width': 1.5,
      },
    },
    {
      selector: 'node[kind = "file"]',
      style: {
        'width': FILE_NODE_WIDTH,
        'height': FILE_NODE_HEIGHT,
        'shape': 'round-rectangle',
        'background-color': dark ? '#111827' : '#ffffff',
        'background-image': 'data(decoration)',
        'background-fit': 'none',
        'background-width': `${FILE_NODE_WIDTH}px`,
        'background-height': `${FILE_NODE_HEIGHT}px`,
        'background-position-x': '50%',
        'background-position-y': '50%',
        'background-repeat': 'no-repeat',
        'border-width': 1,
        'border-color': dark ? '#475569' : '#cbd5e1',
        'label': 'data(label)',
        'font-size': 10.5,
        'font-family': 'system-ui, sans-serif',
        'font-weight': 600,
        'color': dark ? '#f8fafc' : '#1e293b',
        'text-wrap': 'wrap',
        'text-max-width': '132px',
        'text-valign': 'center',
        'text-halign': 'center',
        'text-margin-x': 17,
        'overlay-opacity': 0,
      },
    },
    {
      selector: 'node.missing',
      style: {
        'border-color': '#94a3b8',
        'border-style': 'dashed',
        'background-color': dark ? '#0f172a' : '#f1f5f9',
        'color': '#94a3b8',
        'opacity': 0.78,
      },
    },
    {
      selector: 'node.spec-external',
      style: {
        'border-style': 'dashed',
        'background-color': dark ? '#18181b' : '#fafafa',
      },
    },
    {
      selector: 'edge',
      style: {
        'width': 1.5,
        'curve-style': 'bezier',
        'line-color': dark ? '#64748b' : '#94a3b8',
        'target-arrow-color': dark ? '#64748b' : '#94a3b8',
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.8,
        'label': 'data(label)',
        'font-size': 9,
        'color': dark ? '#cbd5e1' : '#475569',
        'text-background-color': dark ? '#111827' : '#ffffff',
        'text-background-opacity': 0.9,
        'text-background-padding': '2px',
        'overlay-opacity': 0,
      },
    },
    {
      selector: 'node.map-selected',
      style: {
        'outline-color': '#2563eb',
        'outline-width': '5px',
        'outline-offset': '3px',
        'outline-opacity': 1,
        'z-index': 20,
      },
    },
    {
      selector: 'edge.map-selected',
      style: {
        'line-color': '#2563eb',
        'target-arrow-color': '#2563eb',
        'width': 3,
        'z-index': 20,
      },
    },
    {
      selector: 'node.map-related',
      style: {
        'outline-color': '#93c5fd',
        'outline-width': '3px',
        'outline-offset': '2px',
        'outline-opacity': 0.9,
        'z-index': 15,
      },
    },
    {
      selector: 'node.map-filter-muted',
      style: {
        'opacity': 0.38,
      },
    },
  ];
}

function layoutVisibleGraph(graph: Core): void {
  const blocks = GROUP_LAYOUT_ORDER
    .map((groupId) => createGroupBlock(graph, groupId))
    .filter((block): block is GroupBlock => block !== null);

  const topBlocks = blocks.filter((block) =>
    block.id === 'external' || block.id === 'calendar' || block.id === 'support');
  const coreBlock = blocks.find((block) => block.id === 'core') ?? null;
  const lowerBlocks = blocks.filter((block) =>
    !topBlocks.includes(block) && block.id !== 'core');

  graph.batch(() => {
    let y = 0;
    if (topBlocks.length > 0) {
      y += placeBlockRows(topBlocks, y) + GROUP_VERTICAL_GAP;
    }
    if (coreBlock) {
      placeBlock(coreBlock, 0, y);
      y += coreBlock.height + GROUP_VERTICAL_GAP;
    }
    if (lowerBlocks.length > 0) {
      placeBlockRows(lowerBlocks, y);
    }
  });

  graph.fit(graph.elements(':visible'), 34);
}

function refreshGraphTheme(graph: Core): void {
  const dark = document.documentElement.classList.contains('dark');
  graph.nodes('[kind = "file"]').forEach((node) => {
    node.data(
      'decoration',
      fileNodeDecoration(
        node.id(),
        String(node.data('group')),
        node.hasClass('spec-external'),
        String(node.data('status')),
        dark,
      ),
    );
  });
  graph.style(graphStyles()).update();
}

interface GroupBlock {
  id: string;
  nodes: cytoscape.NodeCollection;
  columns: number;
  width: number;
  height: number;
}

function createGroupBlock(graph: Core, groupId: string): GroupBlock | null {
  const nodes = graph
    .getElementById(`group-${groupId}`)
    .children()
    .filter((node) => node.style('display') !== 'none');
  if (nodes.length === 0) return null;

  const columnLimit = groupId === 'core'
    ? 6
    : groupId === 'external'
      ? 3
      : 4;
  const columns = Math.min(columnLimit, nodes.length);
  const rows = Math.ceil(nodes.length / columns);
  const innerGap = 18;
  const horizontalPadding = 44;
  const verticalPadding = 58;
  const contentWidth = columns * FILE_NODE_WIDTH
    + Math.max(0, columns - 1) * innerGap
    + horizontalPadding;
  return {
    id: groupId,
    nodes,
    columns,
    width: groupId === 'core' ? LAYOUT_MAX_WIDTH : contentWidth,
    height: rows * FILE_NODE_HEIGHT + Math.max(0, rows - 1) * innerGap + verticalPadding,
  };
}

function placeBlockRows(blocks: GroupBlock[], startY: number): number {
  let x = 0;
  let y = startY;
  let rowHeight = 0;
  for (const block of blocks) {
    if (x > 0 && x + block.width > LAYOUT_MAX_WIDTH) {
      x = 0;
      y += rowHeight + GROUP_VERTICAL_GAP;
      rowHeight = 0;
    }
    placeBlock(block, x, y);
    x += block.width + GROUP_HORIZONTAL_GAP;
    rowHeight = Math.max(rowHeight, block.height);
  }
  return y - startY + rowHeight;
}

function placeBlock(block: GroupBlock, originX: number, originY: number): void {
  const innerGap = 18;
  const left = originX + 22 + FILE_NODE_WIDTH / 2;
  const top = originY + 36 + FILE_NODE_HEIGHT / 2;
  const coreStep = block.nodes.length > 1
    ? (block.width - 44 - FILE_NODE_WIDTH) / (block.nodes.length - 1)
    : 0;
  block.nodes.forEach((node, index) => {
    const column = index % block.columns;
    const row = Math.floor(index / block.columns);
    node.position({
      x: left + (
        block.id === 'core'
          ? index * coreStep
          : column * (FILE_NODE_WIDTH + innerGap)
      ),
      y: top + row * (FILE_NODE_HEIGHT + innerGap),
    });
  });
}

function getFileIcon(fileId: string, group: string, unknown: boolean): IconNode {
  if (unknown) return FileWarning;
  const directIcons: Record<string, IconNode> = {
    'agency.txt': Building2,
    'routes.txt': Route,
    'trips.txt': BusFront,
    'stop_times.txt': Clock3,
    'stops.txt': MapPin,
    'calendar.txt': CalendarDays,
    'calendar_dates.txt': CalendarCheck2,
    'frequencies.txt': Repeat2,
    'shapes.txt': Waypoints,
    'locations.geojson': MapIcon,
    'transfers.txt': ArrowLeftRight,
    'pathways.txt': Accessibility,
    'levels.txt': Layers,
    'location_groups.txt': Group,
    'location_group_stops.txt': MapPinned,
    'booking_rules.txt': ClipboardCheck,
    'translations.txt': Languages,
    'feed_info.txt': Info,
    'attributions.txt': BadgeCheck,
  };
  const direct = directIcons[fileId];
  if (direct) return direct;
  if (group === 'fare-v1') return Ticket;
  if (group === 'fare-v2') return WalletCards;
  if (group === 'network') return Network;
  if (group === 'geometry') return Shapes;
  return FileText;
}

function fileNodeDecoration(
  fileId: string,
  group: string,
  unknown: boolean,
  status: string,
  dark: boolean,
): string {
  const accentColor = status === 'missing'
    ? cssColor('--color-muted', '#94a3b8')
    : status === 'clean'
      ? NODE_COLORS.LOW
      : NODE_COLORS[status.toUpperCase() as Severity];
  const iconColor = dark
    ? status === 'missing' ? '#64748b' : '#cbd5e1'
    : status === 'missing' ? '#94a3b8' : '#475569';
  return nodeDecoration(
    getFileIcon(fileId, group, unknown),
    accentColor,
    iconColor,
  );
}

function nodeDecoration(
  icon: IconNode,
  accentColor: string,
  iconColor: string,
): string {
  const paths = icon.map(([tag, attributes]) => {
    const serialized = Object.entries(attributes)
      .filter(([key]) => key !== 'key')
      .map(([key, value]) => `${toSvgAttribute(key)}="${String(value)}"`)
      .join(' ');
    return `<${tag} ${serialized}/>`;
  }).join('');
  const svg = `
    <svg xmlns="http://www.w3.org/2000/svg" width="${FILE_NODE_WIDTH}" height="${FILE_NODE_HEIGHT}" viewBox="0 0 ${FILE_NODE_WIDTH} ${FILE_NODE_HEIGHT}">
      <rect x="0" y="0" width="5" height="${FILE_NODE_HEIGHT}" rx="2.5" fill="${accentColor}"/>
      <g transform="translate(15 21) scale(.92)" fill="none" stroke="${iconColor}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        ${paths}
      </g>
    </svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

function toSvgAttribute(attribute: string): string {
  return attribute.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
}

function renderFilePanel(
  panel: HTMLElement,
  summary: FileSummary,
  specUrl: string | undefined,
  onReset: () => void,
): void {
  const grouped = groupNotices(summary.notices);
  const status = summary.missing
    ? t('fileMap.missing')
    : summary.notices.length === 0
      ? t('fileMap.clean')
      : t('fileMap.hasFindings');
  const info = summary.unknown
    ? t('fileMap.nonSpecPresent')
    : summary.info
      ? `${summary.info.rows.toLocaleString()} ${t('fileMap.rows')} - ${formatFileBytes(summary.info.bytes)}`
      : t('fileMap.notPresent');

  panel.innerHTML = `
    <div class="file-map-panel-head">
      <div>
        <span class="file-map-panel-kicker">${t('fileMap.file')}</span>
        <h3>
          ${escHtml(summary.name)}
          ${specUrl
            ? `<a class="file-map-external" href="${specUrl}" target="_blank" rel="noopener noreferrer"
                title="${t('fileMap.openSpec')}" aria-label="${t('fileMap.openSpec')}">&#8599;</a>`
            : ''}
        </h3>
        ${summary.unknown ? `<span class="file-map-non-spec-badge">${t('fileMap.nonSpec')}</span>` : ''}
      </div>
      <span class="file-map-status ${summary.missing ? 'missing' : summary.notices.length ? 'issues' : 'clean'}">${status}</span>
    </div>
    <p class="file-map-meta">${info}</p>
    <div class="file-map-severity-grid">
      ${SEVERITIES.map((severity) => `
        <div>
          <span>${SEVERITY_TR[severity]}</span>
          <strong style="color:${SEVERITY_COLOR[severity]}">${summary.severityCounts[severity].toLocaleString()}</strong>
        </div>
      `).join('')}
    </div>
    <div class="file-map-panel-actions">
      <button class="btn btn-secondary" type="button" id="file-map-reset">
        ${t('fileMap.backToCore')}
      </button>
      <button class="btn btn-primary" type="button" id="file-map-open-findings" ${summary.notices.length === 0 ? 'disabled' : ''}>
        ${t('fileMap.openFindings')}
      </button>
    </div>
    <div class="file-map-finding-list">
      <h4>${t('fileMap.rules')}</h4>
      ${grouped.length > 0
        ? grouped.map(renderRuleGroup).join('')
        : `<p class="file-map-empty">${t('fileMap.noFindings')}</p>`}
    </div>`;

  panel.querySelector('#file-map-reset')?.addEventListener('click', onReset);
  panel.querySelector('#file-map-open-findings')?.addEventListener('click', () => {
    setFixFileFilter(summary.name);
    setPage('fix');
    window.dispatchEvent(new CustomEvent('gtfs-navigate'));
  });
}

function renderOverviewPanel(
  panel: HTMLElement,
  summaryByName: Map<string, FileSummary>,
): void {
  const coreSummaries = CORE_FILE_IDS
    .map((fileId) => summaryByName.get(fileId))
    .filter((summary): summary is FileSummary => summary !== undefined);
  const presentCount = coreSummaries.filter((summary) => summary.info !== null).length;
  const findingCount = coreSummaries.reduce(
    (sum, summary) => sum + summary.notices.length,
    0,
  );
  const missingCount = coreSummaries.filter((summary) => summary.missing).length;

  panel.innerHTML = `
    <div class="file-map-panel-head">
      <div>
        <span class="file-map-panel-kicker">${t('fileMap.overview')}</span>
        <h3>${t('fileMap.coreFiles')}</h3>
      </div>
    </div>
    <p class="file-map-relation-note">${t('fileMap.coreIntro')}</p>
    <p class="file-map-scope-note">${t('fileMap.scopeNote')}</p>
    <div class="file-map-overview-grid">
      <div><span>${t('fileMap.present')}</span><strong>${presentCount}/7</strong></div>
      <div><span>${t('fileMap.findings')}</span><strong>${findingCount.toLocaleString()}</strong></div>
      <div><span>${t('fileMap.missing')}</span><strong>${missingCount}</strong></div>
    </div>
    <p class="file-map-core-hint">${t('fileMap.coreHint')}</p>`;
}

function renderEdgePanel(
  panel: HTMLElement,
  edge: FileMapEdge,
  onReset: () => void,
): void {
  panel.innerHTML = `
    <div class="file-map-panel-head">
      <div>
        <span class="file-map-panel-kicker">${t('fileMap.relationship')}</span>
        <h3>${escHtml(edge.field)}</h3>
      </div>
    </div>
    <div class="file-map-relation">
      <code>${escHtml(edge.source)}</code>
      <span>&rarr;</span>
      <code>${escHtml(edge.target)}</code>
    </div>
    <p class="file-map-relation-note">${t('fileMap.relationshipNote')}</p>
    <button class="btn btn-secondary file-map-reset-wide" type="button" id="file-map-reset">
      ${t('fileMap.backToCore')}
    </button>`;
  panel.querySelector('#file-map-reset')?.addEventListener('click', onReset);
}

interface RuleGroup {
  ruleId: string;
  title: string;
  severity: Severity;
  count: number;
  sample: Notice;
}

function groupNotices(notices: Notice[]): RuleGroup[] {
  const groups = new Map<string, RuleGroup>();
  for (const notice of notices) {
    const current = groups.get(notice.rule_id);
    if (current) {
      current.count += 1;
    } else {
      groups.set(notice.rule_id, {
        ruleId: notice.rule_id,
        title: t(`rule.${notice.rule_id}`) || notice.title,
        severity: notice.severity,
        count: 1,
        sample: notice,
      });
    }
  }
  return [...groups.values()].sort(compareSeverityThenCount);
}

function renderRuleGroup(group: RuleGroup): string {
  return `
    <article class="file-map-rule">
      <div class="file-map-rule-head">
        <code>${escHtml(group.ruleId)}</code>
        <span style="color:${SEVERITY_COLOR[group.severity]}">${SEVERITY_TR[group.severity]}</span>
        <strong>${group.count.toLocaleString()}</strong>
      </div>
      <h5>${escHtml(group.title)}</h5>
      <p>${escHtml(tMsg(group.sample))}</p>
    </article>`;
}

function viewportCenter(element: HTMLElement): { x: number; y: number } {
  return { x: element.clientWidth / 2, y: element.clientHeight / 2 };
}
