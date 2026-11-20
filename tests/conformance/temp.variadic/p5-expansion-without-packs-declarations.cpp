// [temp.variadic]/5: a pack expansion's pattern names at least one pack.
// A base-specifier `A...` ([class.derived.general]/1), a mem-initializer
// `A(1)...` whose class names no pack and a using-declarator `N::f...`
// ([namespace.udecl]/1) are reported at the `...` and kept as if the
// `...` were absent; [class.base.init]/1: only a base-initializer is a
// pack expansion, so `m(1)...` for member `m` is reported too, in a
// template as elsewhere.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 21:13 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 21:29 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 22:29 error "pack expansion for initialization of member 'm'"
//@ diag: 23:53 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 24:36 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 24:59 error "pack expansion for initialization of member 'm'"
//@ diag: 25:26 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 26:22 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 26:21 error "using declaration cannot refer to class member"
struct A { A() {} A(int) {} void f() {} };
struct B : A... { B() : A(1)... {} };
struct C { int m; C() : m(1)... {} };
template <class... Ts> struct D : Ts... { D() : A(1)... {} };
template <class... Ts> struct E : A... { int m; E() : m(1)... {} };
struct G : A { using A::f...; };
void h() { using A::A...; }
B b;
C c;
