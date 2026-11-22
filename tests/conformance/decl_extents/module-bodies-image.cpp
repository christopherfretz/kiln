// The declaration-extent table (spec/ast.md §19.6, §19.9): an importer
// built from module images has the rows replay gives
// (module-bodies-replay.cpp has the same golden, compared as a set of
// rows since the importer numbers its declarations in the image's
// order): the declarations inside the bodies of an inline function, of
// an inline variable's initializer and of a template's pattern
// instantiated here ([module.reach]/1, [module.import]/2) — locals, a
// declarator group, lambdas and their parameters and locals,
// block-scope classes and enumerations, a structured binding
// ([dcl.struct.bind]), if and switch init-statements and conditions
// ([stmt.pre]/1), a requires-expression's parameters
// ([expr.prim.req.general]/2) — and the module declarations
// ([module.unit]/1: exported, with a partition, with attributes) and
// import declarations ([module.import]/1: of a module, of a partition,
// of a header unit, `export import`) of every unit.
//@ module: extents_body:part=Inputs/extents_body_part.cppm
//@ module: extents_body=Inputs/extents_body.cppm
//@ header-unit: "Inputs/extents_hu.h"
//@ two-step
//@ decl-extents: module-bodies.extents unordered
//@ args: -fkiln-decl-extents -fkiln-bmi-image -Wno-experimental-header-units
//@ std: c++20
import extents_body [[]];
import "Inputs/extents_hu.h";
int use() { return body(1) + pattern(2) + split(3) + initialized + from_header(4); }
