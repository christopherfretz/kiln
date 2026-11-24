// A function template's default template argument (spec/ast.md §18.9.2,
// §18.10.2): the row the unit's deduction recorded for `call<dm::S, 4>`
// is carried in its declaration image, and the importer's deduction
// records `call<dm::U, 8>`'s; the dump is the one a replay of the unit
// gives (inst-default-targ-module-replay.cpp has the same golden).
//@ module: default_targ_mod=Inputs/default_targ_mod.cppm
//@ two-step
//@ written-refs: inst-default-targ-module.refs
//@ args: -fkiln-written-refs -fkiln-bmi-image
//@ std: c++20
import default_targ_mod;
int run() { return dm::call<dm::U>() + dm::in_unit(); }
