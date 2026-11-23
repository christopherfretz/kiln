// found-module-image.cpp's unit replayed by the importer (spec/ast.md
// §18.12): the importer records the unit's names, with their found
// sets, by parsing them; the same golden (the two files keep their code
// on the same lines).
//
//@ module: found_mod=Inputs/found_mod.cppm
//@ written-refs: found-module.refs
//@ args: -fkiln-written-refs
//@ std: c++20
import found_mod;
void run() {
  fm::call(1);
  fm::S<int>{}.use(1);
}
