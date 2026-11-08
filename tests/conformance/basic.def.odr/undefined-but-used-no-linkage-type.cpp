// [basic.link]/8, [basic.def.odr]/11: a function or variable with external
// linkage whose type has no linkage (a closure type, a local class of a
// non-inline function, an unnamed class without a typedef name) or
// internal linkage (a class of an unnamed namespace) cannot be defined in
// another translation unit, so odr-using it undefined is ill-formed
// (clang `err_undefined_internal_type`, diagnosed at the end of the
// translation unit at its first declaration, in the order of the first
// uses, with a note at the first use). An odr-used undefined inline
// variable is ill-formed too. Matches clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:25 error "function 'g<(lambda at "
//@ diag: 42:13 note "used here"
//@ diag: 35:25 error "function 'g<(anonymous namespace)::A>' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 43:13 note "used here"
//@ diag: 37:6 error "function 'k' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 43:21 note "used here"
//@ diag: 35:25 error "function 'g<M *>' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 44:31 note "used here"
//@ diag: 39:21 error "variable 'B::sm' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 44:48 note "used here"
//@ diag: 38:10 error "variable 'va' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 45:20 note "used here"
//@ diag: 35:25 error "function 'g<(unnamed"
//@ diag: 45:24 note "used here"
//@ diag: 35:25 error "function 'g<(lambda at "
//@ diag: 45:31 note "used here"
//@ diag: 39:37 error "function 'B::sf' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 46:16 note "used here"
//@ diag: 40:19 error "inline variable 'iv' is not defined"
//@ diag: 47:19 note "used here"
//@ diag: 48:35 error "variable 'lv' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 48:46 note "used here"
namespace { struct A {}; }
template <class F> void g(F);
struct {} us;
void k(A);
extern A va;
struct B { static A sm; static void sf(A); };
extern inline int iv;
auto lam = [] {};
void h1() { g([] {}); }
void h2() { g(A{}); k(A{}); }
void h3() { struct M {}; M m; g(&m); (void)&B::sm; }
void h4() { (void)&va; g(us); g(lam); }
void h5() { B::sf(A{}); B::sf(A{}); }
int h6() { return iv; }
void h7() { struct L {}; extern L lv; (void)&lv; }
