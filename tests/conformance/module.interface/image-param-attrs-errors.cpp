// [dcl.attr.grammar]/5, [class.temporary]/6, [module.interface],
// spec/modules.md §11.11 (WP-1314): an imported function's
// `[[clang::lifetimebound]]` parameters diagnose the importer's dangling
// references exactly as with the replayed source, under `--bmi-image`.
//@ expect: compile-fail
//@ std: c++26
//@ module: imgpa=Inputs/img-param-attrs.cppm
//@ diag: 10:37 error "returning reference to local temporary object"
import imgpa;
const int &bad1() { return pa::pick(1, 0); }
int main() { return 0; }
