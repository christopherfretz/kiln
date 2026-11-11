// [temp.dep.candidate], [dcl.init.general]/16, [module.interface],
// spec/modules.md §11.11 "Templates as patterns" (WP-1279): an
// importer's errors against patterns with unresolved names built from
// the declaration image are the ones against the replayed source (errors
// inside an instantiated pattern, located in the module, are pinned in
// crates/kiln_driver/tests/modules_bmi.rs).
//@ expect: compile-fail
//@ std: c++20
//@ module: imgu=Inputs/img-lookup.cppm
//@ strict-diags
//@ diag: 15:7 error "cannot initialize a variable of type 'void *' with an rvalue of type 'int'"
//@ diag: 16:6 error "no viable conversion from 'S' to 'int *'"
//@ diag: 17:9 error "no matching function for call to 'next'"
import imgu;
void *a = iu::call(1);
int *b = iu::plus(iu::S{1}, iu::S{2});
int c = iu::next<iu::S>();
