// template-id-module-image.cpp's unit replayed by the importer
// (spec/ast.md §18.5): the importer records the unit's names by parsing
// them; the same golden (the two files keep their code on the same
// lines).
//@ module: tid_mod=Inputs/tid_mod.cppm
//
//@ written-refs: template-id-module.refs
//@ args: -fkiln-written-refs
//@ std: c++20
import tid_mod;
int run() { return tm::g(1) + tm::S<int>{}.a() + tm::v<int>; }
