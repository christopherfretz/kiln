// [basic.link]/15.1, 15.2, 16.1, 16.5 (WP-1856): /15 ignores a variable's
// initializer but not the variable's type, and a non-inline function's body
// but not its deduced return type. So in a module interface a variable whose
// placeholder type deduces to a TU-local type, or a function whose return
// type is deduced to one, is an exposure. That includes a local class of a
// specialization that is TU-local because its definition exposes a TU-local
// entity (/16.5 makes the specialization TU-local, /16.1 its local class);
// the check waits for the unit's end, when it is known which
// specializations are. A closure type is a local class too. GCC 16.1
// rejects each of these (for the specializations it reports the local class
// or its member, once each); clang 22 accepts all of it (warning at `te(3)`).
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 does not check deduced types
//@ strict-diags
//@ args: -Wno-TU-local-entity-exposure
//@ diag: 38:13 error "declaration of 'w1' exposes TU-local entity 'S'"
//@ diag: 39:13 error "declaration of 'w2' exposes TU-local entity 'S'"
//@ diag: 40:14 error "declaration of 'w3' exposes TU-local entity 'S'"
//@ diag: 41:23 error "declaration of 'w4' exposes TU-local entity 'S'"
//@ diag: 42:20 error "declaration of 'w5' exposes TU-local entity 'S'"
//@ diag: 43:13 error "declaration of 'f3' exposes TU-local entity 'S'"
//@ diag: 44:13 error "declaration of 'v1' exposes TU-local entity 'L'"
//@ diag: 45:13 error "declaration of 'v2' exposes TU-local entity 'L'"
//@ diag: 46:13 error "declaration of 'v3' exposes TU-local entity '(lambda at "
//@ diag: 47:24 error "declaration of 'v6' exposes TU-local entity 'L'"
//@ diag: 48:20 error "declaration of 'v7' exposes TU-local entity 'L'"
//@ diag: 49:14 error "declaration of 'v8' exposes TU-local entity 'L'"
//@ diag: 50:13 error "declaration of 'f1' exposes TU-local entity 'L'"
//@ diag: 51:13 error "declaration of 'f2' exposes TU-local entity 'L'"
export module M;
static int helper() { return 1; }
namespace { struct S { int v; }; }
static S mk() { return S(); }
template <class T> auto te(T) { struct L { constexpr int g() { return helper(); } }; return L(); }
template <class T> auto tf(T) { struct L { S s; }; return L(); }
template <class T> auto tl(T) { return [] { return helper(); }; }
export auto w1 = mk();
export auto w2 = S();
export auto *w3 = new S();
export decltype(auto) w4 = mk();
export const auto &w5 = S();
export auto f3() { return S(); }
export auto v1 = te(1);
export auto v2 = tf(1);
export auto v3 = tl(1);
export decltype(te(2)) v6;
export inline auto v7 = te(3);
export auto *v8 = new auto(tf(4));
export void f1(decltype(tf(5)));
export auto f2() { return te(6); }
