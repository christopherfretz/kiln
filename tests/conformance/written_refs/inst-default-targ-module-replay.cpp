// inst-default-targ-module-image.cpp's unit replayed by the importer
// (spec/ast.md §18.9.2): the importer records the unit's names and the
// rows of its deduction by parsing it; the same golden (the two files
// keep their code on the same lines).
//
//
//@ module: default_targ_mod=Inputs/default_targ_mod.cppm
//@ written-refs: inst-default-targ-module.refs
//@ args: -fkiln-written-refs
//@ std: c++20
import default_targ_mod;
int run() { return dm::call<dm::U>() + dm::in_unit(); }
