// Stage 5 (spec/ast.md §18.10.2): an importer that replays the module
// unit records the unit's names by parsing them, in its own table;
// module-image.cpp, built from the image, has the same golden (the
// two files keep their code on the same lines). The importer's
// instantiation of the unit's template records its rows in its own
// table here too.
//
//@ module: refs_mod=Inputs/refs_mod.cppm
//@ written-refs: module.refs
//@ args: -fkiln-written-refs
//@ std: c++20
import refs_mod;
int use() { return geo::twice(2) + top(1) + geo::measure(geo::Pt{1, 2}); }
