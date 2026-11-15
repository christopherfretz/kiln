// [dcl.fct.def.general] A GNU attribute GCC knows (one with a `gnu::`
// spelling) after the declarator of a function definition with a body
// warns that GCC does not allow it there (clang `ParseFunctionDefinition`,
// `-Wgcc-compat`, on by default); not on a declaration, a deleted
// definition, an in-class member definition, a clang-only attribute or an
// unknown one. Found building SIMD code with `target` attributes. WP-1426.
//@ expect: compile-ok
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 20:26 warning "GCC does not allow 'unused' attribute in this position on a function definition"
//@ diag: 21:26 warning "GCC does not allow 'always_inline' attribute in this position on a function definition"
//@ diag: 22:26 warning "GCC does not allow 'deprecated' attribute in this position on a function definition"
//@ diag: 23:26 warning "GCC does not allow 'unused' attribute in this position on a function definition"
//@ diag: 23:34 warning "GCC does not allow 'cold' attribute in this position on a function definition"
//@ diag: 24:44 warning "GCC does not allow 'noinline' attribute in this position on a function definition"
//@ diag: 25:26 warning "unknown attribute 'bogus' ignored"
//@ diag: 26:40 warning "GCC does not allow 'cold' attribute in this position on a function definition"
//@ diag: 27:26 warning "GCC does not allow 'cold' attribute in this position on a function definition"
//@ diag: 30:26 warning "GCC does not allow 'target' attribute in this position on a function definition"
void f1() __attribute__((unused)) {}
void f2() __attribute__((always_inline)) {}
void f3() __attribute__((deprecated)) {}
void f4() __attribute__((unused, cold)) {}
template<class T> void f5() __attribute__((noinline)) {}
void f7() __attribute__((bogus)) {}
namespace N { void f8() __attribute__((cold)) {} }
void f9() __attribute__((cold)) try {} catch(...) {}
void f10() __attribute__((cold)) = delete;
int f11(int x) __attribute__((enable_if(x > 0, ""))) { return x; }
void g1() __attribute__((target("sve"))) {}
void g2() __attribute__((cold));
struct S { void m() __attribute__((noinline)) {} };
