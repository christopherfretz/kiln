// Substitution keys of a replayed module unit (spec/ast.md §18.14.5): an
// importer that replays the unit records its matches by parsing it, in
// its own table; subst-partial-module-image.cpp, built from the image,
// has the same golden (the two files keep their code on the same
// lines).
//
//
//@ module: subst_partial_mod=Inputs/subst_partial_mod.cppm
//@ written-refs: subst-partial-module.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
//
import subst_partial_mod;
int use() { return sm::W<sm::S, int *, int>::v + sm::in_unit(); }
