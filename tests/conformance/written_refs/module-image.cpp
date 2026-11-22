// Stage 5 (spec/ast.md §18.10.2): a module file's image carries the
// written-reference table of the unit, and an importer that records
// merges it into its own table, so its dump is the one a replay of the
// unit gives (module-replay.cpp has the same golden). The importer's
// instantiation of the unit's template records its rows in its own
// table.
//@ module: refs_mod=Inputs/refs_mod.cppm
//@ two-step
//@ written-refs: module.refs
//@ args: -fkiln-written-refs -fkiln-bmi-image
//@ std: c++20
import refs_mod;
int use() { return geo::twice(2) + top(1) + geo::measure(geo::Pt{1, 2}); }
