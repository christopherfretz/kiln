// [temp.deduct]/2, [temp.arg.nontype]/1, [temp.variadic], [module.interface],
// spec/modules.md §11.11 "Templates as patterns" (WP-1279): an importer's
// errors against variadic templates and non-type default template
// arguments built from the declaration image are the ones against the
// replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgp=Inputs/img-pack.cppm
//@ strict-diags
//@ diag: 16:9 error "no matching function for call to 'sz'"
//@ diag: 17:9 error "no matching function for call to 'def'"
//@ diag: 18:9 error "no matching function for call to 'first'"
//@ diag: 19:5 error "too many template arguments for class template 'Arr'"
import imgp;
struct S {};
int a = ip::sz<S, 1, 2>();
int b = ip::def<S>();
int c = ip::first();
ip::Arr<int, 2, 3> d;
