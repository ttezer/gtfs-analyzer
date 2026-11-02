//! Guard: every WASM orchestrator must hand the K1 GeoJSON locations to the K3 entity map.
//!
//! The native pipeline (`crates/pipeline/src/lib.rs`) copies `geojson_location_ids` and
//! `geojson_geometries` into `EntityMap`; the WASM orchestrators did not. In the browser and
//! `gtfs-sdk` the set was empty, so XFL_025 reported every Flex `location_id` as undefined,
//! XFL_031 could never fire and STM_060 never ran. The parity fixtures contain no Flex
//! locations, so this guard checks the call sites in source instead.

const SRC: &str = include_str!("../src/lib.rs");

#[test]
fn every_entity_map_build_receives_geojson_locations() {
    let code: String = SRC
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let builds = code.matches("build_entity_map(&k2.records)").count();
    assert!(
        builds > 0,
        "no build_entity_map call found; update this guard"
    );
    for field in ["geojson_location_ids", "geojson_geometries"] {
        let wired = code
            .matches(&format!("k3.entity_map.{field} = k1.{field}.clone();"))
            .count();
        assert_eq!(
            wired, builds,
            "{builds} entity-map builds but {wired} copy `{field}` from K1"
        );
    }
}
