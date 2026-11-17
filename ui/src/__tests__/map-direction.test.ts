import { describe, expect, it, vi } from 'vitest';
import { routeDirectionShapeId } from '../pages/producer';
import type { ValidationResult } from '../types';

vi.mock('../state', () => ({ setPage: vi.fn(), getState: () => ({ agencyFilter: [] }), setAgencyFilter: vi.fn() }));
vi.mock('../hidden-rules', () => ({
  renderHiddenRulesBar: () => '',
  attachHiddenRuleListeners: () => undefined,
}));

// The engine writes shape_routes directions as the raw direction_id ("0"/"1"), the same values as
// trip_directions. They used to be "Gidiş"/"Dönüş", so a trip without its own shape never found
// its route's line in its direction and the map drew straight lines between stops.
describe('route shape by direction', () => {
  const ni = {
    route_shapes: { R1: ['OUT', 'IN'] },
    shape_routes: { OUT: [['R1', '0']], IN: [['R1', '1']] },
  } as unknown as ValidationResult['name_index'];

  it('picks the shape drawn in the trip direction', () => {
    expect(routeDirectionShapeId(ni, 'R1', '1')).toBe('IN');
    expect(routeDirectionShapeId(ni, 'R1', '0')).toBe('OUT');
  });

  it('falls back to any shape of the route when the direction is unknown', () => {
    expect(routeDirectionShapeId(ni, 'R1', '')).toBe('OUT');
    expect(routeDirectionShapeId(ni, 'R1', undefined)).toBe('OUT');
  });

  it('finds nothing for a direction the route has no shape for', () => {
    const oneWay = { ...ni, route_shapes: { R1: ['OUT'] } } as unknown as ValidationResult['name_index'];
    expect(routeDirectionShapeId(oneWay, 'R1', '1')).toBeUndefined();
  });
});
