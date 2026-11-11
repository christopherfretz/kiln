// GNU old-style field designators in C++ (an extension; [dcl.init.aggr]):
// `name: value` in a braced initializer means `.name = value` (clang
// `ParseInitializerWithPotentialDesignator`), so it is checked like a
// C++20 designator: out of declaration order draws -Wreorder-init-list,
// and an unknown name is an error. A braced value is allowed.
//@ std: c++20
//@ args: -Wgnu-designator
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:9 warning "use of GNU old-style field designator extension"
//@ diag: 20:9 warning "use of GNU old-style field designator extension"
//@ diag: 20:15 warning "use of GNU old-style field designator extension"
//@ diag: 20:18 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 20:12 note "previous initialization for field 'b' is here"
//@ diag: 21:9 warning "use of GNU old-style field designator extension"
//@ diag: 21:9 error "field designator 'e' does not refer to any field in type 'S'"
//@ diag: 22:36 warning "use of GNU old-style field designator extension"
struct S { int a, b; int c[2]; };
S w = { c: {1, 2} };
S x = { b: 1, a: 2 };
S y = { e: 3 };
union U { double d; int i; } u = { i: 4 };
