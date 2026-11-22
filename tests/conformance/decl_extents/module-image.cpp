// The declaration-extent table (spec/ast.md §19.6): a module file's
// declaration image carries the rows of its records and their
// parameters, so an importer that builds the module from the image has
// the rows replay would give (module-replay.cpp has the same golden).
//@ module: extents_mod=Inputs/extents_mod.cppm
//@ two-step
//@ decl-extents: module.extents
//@ args: -fkiln-decl-extents -fkiln-bmi-image
//@ std: c++20
import extents_mod;
int use() { return geo::twice(2) + top(1); }
