// Substitution keys through a module file's image (spec/ast.md §18.10.2,
// §18.14.5): the unit instantiates `W<S, int *, int>`, which both partial
// specializations match ([temp.spec.partial.match]/1.3); the image
// carries both keys whole, with their rows and roots, and the importer's
// use of the same specialization adds none. The dump is the one a replay
// of the unit gives (subst-partial-module-replay.cpp has the same
// golden).
//@ module: subst_partial_mod=Inputs/subst_partial_mod.cppm
//@ two-step
//@ written-refs: subst-partial-module.refs
//@ args: -fkiln-written-ref-keys -fkiln-bmi-image
//@ std: c++26
import subst_partial_mod;
int use() { return sm::W<sm::S, int *, int>::v + sm::in_unit(); }
