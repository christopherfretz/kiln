// Stage 5 (spec/ast.md §18.10.2): a module image carries the holders of
// an alias, an enumerator, a template head and a static_assert
// (§18.3), and the declarator groups of shared decl-specifiers
// (§18.3.1); the importer's dump equals a replay's
// (enclosing-module-replay.cpp has the same golden; [module.import]).
//@ module: enclosing_mod=Inputs/enclosing_mod.cppm
//@ two-step
//@ written-refs: enclosing-module.refs
//@ args: -fkiln-written-refs -fkiln-bmi-image
//@ std: c++20
import enclosing_mod;
int use() { return shapes::make().x + shapes::second.y; }
