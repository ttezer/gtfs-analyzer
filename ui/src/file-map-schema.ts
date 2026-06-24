export type FileMapGroup =
  | 'core'
  | 'calendar'
  | 'geometry'
  | 'station'
  | 'fare-v1'
  | 'fare-v2'
  | 'network'
  | 'flex'
  | 'support'
  | 'external';

export interface FileMapNode {
  id: string;
  group: FileMapGroup;
  x: number;
  y: number;
  specUrl: string;
}

export interface FileMapEdge {
  id: string;
  source: string;
  target: string;
  field: string;
}

const SPEC_BASE = 'https://gtfs.org/documentation/schedule/reference/';

export const FILE_MAP_GROUPS: Record<FileMapGroup, string> = {
  core: 'Core service',
  calendar: 'Calendar',
  geometry: 'Geometry',
  station: 'Station access',
  'fare-v1': 'Fares v1',
  'fare-v2': 'Fares v2',
  network: 'Areas & networks',
  flex: 'Flex',
  support: 'Supporting files',
  external: 'Non-spec files',
};

export const CORE_FILE_IDS = [
  'agency.txt',
  'routes.txt',
  'trips.txt',
  'stop_times.txt',
  'stops.txt',
  'calendar.txt',
  'calendar_dates.txt',
] as const;

export const CORE_FILE_ID_SET = new Set<string>(CORE_FILE_IDS);

function node(
  id: string,
  group: FileMapGroup,
  x: number,
  y: number,
  anchor = id.replace('.', '').replace('_', ''),
): FileMapNode {
  return {
    id,
    group,
    x: Math.round(x * 0.84),
    y: Math.round(y * 0.62),
    specUrl: `${SPEC_BASE}#${anchor}`,
  };
}

export const FILE_MAP_NODES: readonly FileMapNode[] = [
  node('agency.txt', 'core', 80, 300, 'agencytxt'),
  node('routes.txt', 'core', 330, 300, 'routestxt'),
  node('trips.txt', 'core', 580, 300, 'tripstxt'),
  node('stop_times.txt', 'core', 830, 300, 'stop_timestxt'),
  node('stops.txt', 'core', 1080, 300, 'stopstxt'),
  node('frequencies.txt', 'core', 580, 470, 'frequenciestxt'),

  node('calendar.txt', 'calendar', 430, 70, 'calendartxt'),
  node('calendar_dates.txt', 'calendar', 700, 70, 'calendar_datestxt'),

  node('shapes.txt', 'geometry', 760, 540, 'shapestxt'),
  node('locations.geojson', 'geometry', 1020, 540, 'locationsgeojson'),

  node('transfers.txt', 'station', 1230, 130, 'transferstxt'),
  node('pathways.txt', 'station', 1430, 130, 'pathwaystxt'),
  node('levels.txt', 'station', 1430, 300, 'levelstxt'),

  node('fare_attributes.txt', 'fare-v1', 80, 760, 'fare_attributestxt'),
  node('fare_rules.txt', 'fare-v1', 330, 760, 'fare_rulestxt'),

  node('fare_media.txt', 'fare-v2', 80, 1010, 'fare_mediatxt'),
  node('rider_categories.txt', 'fare-v2', 330, 1010, 'rider_categoriestxt'),
  node('fare_products.txt', 'fare-v2', 580, 1010, 'fare_productstxt'),
  node('fare_leg_rules.txt', 'fare-v2', 830, 1010, 'fare_leg_rulestxt'),
  node('timeframes.txt', 'fare-v2', 1080, 1010, 'timeframestxt'),
  node('fare_transfer_rules.txt', 'fare-v2', 1330, 1010, 'fare_transfer_rulestxt'),
  node('fare_leg_join_rules.txt', 'fare-v2', 1080, 1170, 'fare_leg_join_rulestxt'),

  node('areas.txt', 'network', 80, 1270, 'areastxt'),
  node('stop_areas.txt', 'network', 330, 1270, 'stop_areastxt'),
  node('networks.txt', 'network', 580, 1270, 'networkstxt'),
  node('route_networks.txt', 'network', 830, 1270, 'route_networkstxt'),

  node('location_groups.txt', 'flex', 1180, 720, 'location_groupstxt'),
  node('location_group_stops.txt', 'flex', 1430, 720, 'location_group_stopstxt'),
  node('booking_rules.txt', 'flex', 1430, 880, 'booking_rulestxt'),

  node('translations.txt', 'support', 1030, 1450, 'translationstxt'),
  node('feed_info.txt', 'support', 1280, 1450, 'feed_infotxt'),
  node('attributions.txt', 'support', 1530, 1450, 'attributionstxt'),
] as const;

export const FILE_MAP_EDGES: readonly FileMapEdge[] = [
  { id: 'routes-agency', source: 'routes.txt', target: 'agency.txt', field: 'agency_id' },
  { id: 'trips-routes', source: 'trips.txt', target: 'routes.txt', field: 'route_id' },
  { id: 'trips-calendar', source: 'trips.txt', target: 'calendar.txt', field: 'service_id' },
  { id: 'trips-calendar-dates', source: 'trips.txt', target: 'calendar_dates.txt', field: 'service_id' },
  { id: 'stop-times-trips', source: 'stop_times.txt', target: 'trips.txt', field: 'trip_id' },
  { id: 'stop-times-stops', source: 'stop_times.txt', target: 'stops.txt', field: 'stop_id' },
  { id: 'trips-shapes', source: 'trips.txt', target: 'shapes.txt', field: 'shape_id' },
  { id: 'frequencies-trips', source: 'frequencies.txt', target: 'trips.txt', field: 'trip_id' },
  { id: 'transfers-stops', source: 'transfers.txt', target: 'stops.txt', field: 'from/to_stop_id' },
  { id: 'pathways-stops', source: 'pathways.txt', target: 'stops.txt', field: 'from/to_stop_id' },
  { id: 'stops-levels', source: 'stops.txt', target: 'levels.txt', field: 'level_id' },
  { id: 'fare-rules-fare-attributes', source: 'fare_rules.txt', target: 'fare_attributes.txt', field: 'fare_id' },
  { id: 'fare-rules-routes', source: 'fare_rules.txt', target: 'routes.txt', field: 'route_id' },
  { id: 'fare-products-media', source: 'fare_products.txt', target: 'fare_media.txt', field: 'fare_media_id' },
  { id: 'fare-products-riders', source: 'fare_products.txt', target: 'rider_categories.txt', field: 'rider_category_id' },
  { id: 'fare-leg-products', source: 'fare_leg_rules.txt', target: 'fare_products.txt', field: 'fare_product_id' },
  { id: 'fare-leg-networks', source: 'fare_leg_rules.txt', target: 'networks.txt', field: 'network_id' },
  { id: 'fare-leg-areas', source: 'fare_leg_rules.txt', target: 'areas.txt', field: 'from/to_area_id' },
  { id: 'timeframes-leg', source: 'timeframes.txt', target: 'fare_leg_rules.txt', field: 'timeframe_group_id' },
  { id: 'fare-transfer-products', source: 'fare_transfer_rules.txt', target: 'fare_products.txt', field: 'fare_product_id' },
  { id: 'fare-join-networks', source: 'fare_leg_join_rules.txt', target: 'networks.txt', field: 'from/to_network_id' },
  { id: 'stop-areas-areas', source: 'stop_areas.txt', target: 'areas.txt', field: 'area_id' },
  { id: 'stop-areas-stops', source: 'stop_areas.txt', target: 'stops.txt', field: 'stop_id' },
  { id: 'route-networks-routes', source: 'route_networks.txt', target: 'routes.txt', field: 'route_id' },
  { id: 'route-networks-networks', source: 'route_networks.txt', target: 'networks.txt', field: 'network_id' },
  { id: 'location-group-stops-groups', source: 'location_group_stops.txt', target: 'location_groups.txt', field: 'location_group_id' },
  { id: 'location-group-stops-stops', source: 'location_group_stops.txt', target: 'stops.txt', field: 'stop_id' },
  { id: 'stop-times-groups', source: 'stop_times.txt', target: 'location_groups.txt', field: 'location_group_id' },
  { id: 'stop-times-geojson', source: 'stop_times.txt', target: 'locations.geojson', field: 'location_id' },
  { id: 'stop-times-booking', source: 'stop_times.txt', target: 'booking_rules.txt', field: 'booking_rule_id' },
  { id: 'attributions-agency', source: 'attributions.txt', target: 'agency.txt', field: 'agency_id' },
  { id: 'attributions-routes', source: 'attributions.txt', target: 'routes.txt', field: 'route_id' },
  { id: 'attributions-trips', source: 'attributions.txt', target: 'trips.txt', field: 'trip_id' },
] as const;

export function getDirectlyRelatedFileIds(fileId: string): Set<string> {
  const related = new Set<string>([fileId]);
  for (const edge of FILE_MAP_EDGES) {
    if (edge.source === fileId) related.add(edge.target);
    if (edge.target === fileId) related.add(edge.source);
  }
  return related;
}

export function getVisibleFileIdsForSelection(
  fileId: string,
  baseVisibleIds: Iterable<string>,
  hasFindings: (candidateId: string) => boolean,
): Set<string> {
  const visible = new Set<string>(baseVisibleIds);
  for (const relatedId of getDirectlyRelatedFileIds(fileId)) {
    if (
      relatedId === fileId
      || CORE_FILE_ID_SET.has(relatedId)
      || hasFindings(relatedId)
    ) {
      visible.add(relatedId);
    }
  }
  return visible;
}

export function getBaseVisibleFileIds(
  files: Iterable<{ id: string; hasFindings: boolean; unknown: boolean }>,
): Set<string> {
  const visible = new Set<string>(CORE_FILE_IDS);
  for (const file of files) {
    if (file.hasFindings || file.unknown) visible.add(file.id);
  }
  return visible;
}

export function validateFileMapSchema(): string[] {
  const errors: string[] = [];
  const nodeIds = new Set<string>();
  for (const mapNode of FILE_MAP_NODES) {
    if (nodeIds.has(mapNode.id)) errors.push(`Duplicate node: ${mapNode.id}`);
    nodeIds.add(mapNode.id);
  }
  const edgeIds = new Set<string>();
  for (const edge of FILE_MAP_EDGES) {
    if (edgeIds.has(edge.id)) errors.push(`Duplicate edge: ${edge.id}`);
    edgeIds.add(edge.id);
    if (!nodeIds.has(edge.source)) errors.push(`Missing source: ${edge.source}`);
    if (!nodeIds.has(edge.target)) errors.push(`Missing target: ${edge.target}`);
  }
  return errors;
}
