// [temp.expl.spec], [temp.spec.partial.match], [dcl.pre]/10,
// [module.interface], spec/modules.md §11.11 "Templates as patterns"
// (WP-1307): an importer's static assertions see the values of a
// module's explicit and partial specializations of variable templates,
// from the declaration image as from the replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgvs=Inputs/img-var-spec.cppm
//@ diag: 12:15 error "static assertion failed"
//@ diag: 13:15 error "static assertion failed"
import imgvs;
static_assert(vs::kind<char> == 8);
static_assert(vs::same<int *, long *>);
static_assert(vs::kind<char *> == 8 && vs::same<char, char>);
