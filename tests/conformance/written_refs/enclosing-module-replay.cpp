// Stage 5 (spec/ast.md §18.10.2): an importer that replays the module
// unit records its holders (§18.3) and declarator groups (§18.3.1) by
// parsing it; enclosing-module-image.cpp, built from the image, has the
// same golden (the two files keep their code on the same lines;
// [module.import]).
//@ module: enclosing_mod=Inputs/enclosing_mod.cppm
//@ written-refs: enclosing-module.refs
//@ args: -fkiln-written-refs
//@ std: c++20
//
import enclosing_mod;
int use() { return shapes::make().x + shapes::second.y; }
