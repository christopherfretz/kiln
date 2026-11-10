// [module.interface], [class.virtual], [class.dtor], spec/modules.md §7
// (WP-1198): vtable-alias-O1-run.cpp with `main` on the import's line,
// which kiln used to drop with the rest of that line (the import
// directive ends at its `;`, [cpp.import]).
//@ expect: run
//@ std: c++20
//@ args: -O1
//@ module: vta=Inputs/vtable-alias.cppm
//@ exit: 4
import vta; int main() { A a; return a.f(); }
