// The found sets of a module unit's templates (spec/ast.md §18.12,
// §18.10.2) are carried in its declaration image; the importer's dump
// is the one a replay of the unit gives (found-module-replay.cpp has the
// same golden).
//@ module: found_mod=Inputs/found_mod.cppm
//@ two-step
//@ written-refs: found-module.refs
//@ args: -fkiln-written-refs -fkiln-bmi-image
//@ std: c++20
import found_mod;
void run() {
  fm::call(1);
  fm::S<int>{}.use(1);
}
