// The entries of a module unit's dependent template-ids (spec/ast.md
// §18.5, §18.10.2) are carried in its declaration image; the importer's
// dump is the one a replay of the unit gives
// (template-id-module-replay.cpp has the same golden).
//@ module: tid_mod=Inputs/tid_mod.cppm
//@ two-step
//@ written-refs: template-id-module.refs
//@ args: -fkiln-written-refs -fkiln-bmi-image
//@ std: c++20
import tid_mod;
int run() { return tm::g(1) + tm::S<int>{}.a() + tm::v<int>; }
