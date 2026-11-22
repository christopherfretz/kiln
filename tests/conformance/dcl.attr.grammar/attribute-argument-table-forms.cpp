// [dcl.attr.grammar]/4 (GNU extension): the argument clause of an attribute
// clang knows is parsed as clang's ParseAttributeArgsCommon parses it: a
// missing clause, a braced or packed argument, a missing comma; then its
// count is checked against the generated table (spec/parse.md §6.2
// "Argument table", WP-1879). Verified against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:16 error "'section' attribute takes one argument"
//@ diag: 23:21 error "use of undeclared identifier 'f1'"
//@ diag: 23:16 error "'leaf' attribute takes no arguments"
//@ diag: 24:21 error "use of undeclared identifier 'kiln_undeclared'"
//@ diag: 25:16 error "'cold' attribute takes no arguments"
//@ diag: 26:45 error "attribute 'cold' does not support argument pack expansion"
//@ diag: 27:41 error "expression contains unexpanded parameter pack 'I'"
//@ diag: 28:24 error "expected ')'"
//@ diag: 28:16 error "'cold' attribute takes no arguments"
//@ diag: 29:33 error "expected ')'"
//@ diag: 29:33 error "expected ')'"
//@ diag: 29:37 error "expected function body after function declarator"
//@ diag: 30:29 error "'cold' attribute takes no arguments"

__attribute__((section)) void f13();
__attribute__((leaf(f1()))) void f14();
__attribute__((cold(kiln_undeclared))) void f15();
__attribute__((cold({1, 2}))) void f16();
template <int... I> __attribute__((cold(I...))) void f17();
template <int... I> __attribute__((cold(I))) void f18();
__attribute__((cold(1)), hot) void f19();
void f20() __attribute__((cold(1 2)));
struct S21 { __attribute__((cold(1))) void m(); };
