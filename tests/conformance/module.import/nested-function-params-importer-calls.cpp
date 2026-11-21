// [module.import]/2, [clang ext] [temp.deduct.general]/8 (WP-1859): an
// importer's calls of imported function templates whose nested function
// types get a parameter qualified with an address space, or a Swift
// context parameter that is not a pointer, find no viable function (the
// substitution failure is noted at the template in the module), and the
// member alias of an imported class template is checked when the
// importer instantiates it (the error is in the module, at the
// parameter). Valid arguments still call them.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ module: nfp=Inputs/nested-function-params.cppm
//@ diag: 20:3 error "no matching function for call to 'a2'"
//@ diag: 21:3 error "no matching function for call to 'h4'"
//@ diag: 23:3 error "no matching function for call to 'a1'"
import nfp;
#define AS(n) __attribute__((address_space(n)))

void f() {
  a2<int AS(1)>(nullptr);
  h4<int>();
  S<int>::F x;
  a1<2>();
  a2<int>(nullptr);
  h4<int *>();
}
