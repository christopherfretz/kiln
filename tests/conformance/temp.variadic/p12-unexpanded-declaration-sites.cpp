// [temp.variadic]/12: a pack named outside an expansion in a template's
// declarations is diagnosed when the template is defined, at the operand
// (clang's DiagnoseUnexpandedParameterPack contexts): a base-specifier's
// type ([class.derived.general]/1), an enumerator value ([dcl.enum]/2),
// a using-declarator's nested-name-specifier at class and block scope
// ([namespace.udecl]/1), a bit-field width ([class.bit]/1), an enum's
// fixed underlying type ([dcl.enum]/2), an explicit-specifier and a
// noexcept-specifier operand, a placeholder's type-constraint
// ([dcl.spec.auto.general]/2), a mem-initializer's class or initializer
// ([class.base.init]/1, at its `(` or `{`) and an attribute's argument
// ([dcl.attr.grammar]/4).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 29:35 error "base type contains unexpanded parameter pack 'Ts'"
//@ diag: 30:46 error "enumerator value contains unexpanded parameter pack 'E'"
//@ diag: 31:41 error "using declaration contains unexpanded parameter pack 'Ts'"
//@ diag: 32:41 error "using declaration contains unexpanded parameter pack 'Ts'"
//@ diag: 33:41 error "bit-field size contains unexpanded parameter pack 'N'"
//@ diag: 34:45 error "fixed underlying type contains unexpanded parameter pack 'Ts'"
//@ diag: 35:42 error "expression contains unexpanded parameter pack 'N'"
//@ diag: 36:40 error "expression contains unexpanded parameter pack 'N'"
//@ diag: 38:47 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 40:51 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 41:45 error "initializer contains unexpanded parameter pack 'N'"
//@ diag: 42:48 error "initializer contains unexpanded parameter pack 'N'"
//@ diag: 43:48 error "initializer contains unexpanded parameter pack 'N'"
//@ diag: 44:48 error "expression contains unexpanded parameter pack 'N'"
template <class... Ts> struct S : Ts {};
template <int... E> struct En { enum e { x = E }; };
template <class... Ts> struct U { using Ts::f; };
template <class... Ts> void g() { using Ts::f; }
template <int... N> struct Bf { int b : N; };
template <class... Ts> struct Fx { enum E : Ts { x }; };
template <int... N> struct Ex { explicit(N) Ex(); };
template <int... N> void ne() noexcept(N);
template <class T, class... Ts> concept K = true;
template <class... Ts> void ca() { K<Ts> auto x = 1; }
struct A { A(int); };
template <class... Ts> struct M1 : Ts... { M1() : Ts(1) {} };
template <int... N> struct M2 : A { M2() : A(N) {} };
template <int... N> struct M3 { int m; M3() : m(N) {} };
template <int... N> struct M4 { int m; M4() : m{N} {} };
template <int... N> struct At { [[gnu::aligned(N)]] char c; };
// Expanded, the same sites are valid.
template <class... Ts> struct S2 : Ts... {};
template <int... E> struct En2 { enum e { x = (E + ... + 0) }; };
template <class... Ts> struct U2 : Ts... { using Ts::f...; };
template <int... N> struct Bf2 { int b : (N + ...); };
template <int... N> struct Ex2 { explicit((N && ...)) Ex2(); };
template <int... N> void ne2() noexcept((N && ...));
template <class... Ts> void ca2() { K<Ts...> auto x = 1; }
template <class... Ts> struct M5 : Ts... { M5() : Ts(1)... {} };
template <int... N> struct M6 : A { M6() : A((N + ... + 0)) {} };
template <int... N> struct At2 { [[gnu::aligned((N + ...))]] char c; };
struct F1 { int f(int) { return 1; } };
struct F2 { int f(double) { return 2; } };
struct B1 { B1(int) {} };
struct B2 { B2(int) {} };
S2<F1, F2> s2;
En2<1, 2> en2;
U2<F1, F2> u2;
Bf2<1, 2> bf2;
M5<B1, B2> m5;
M6<1, 2> m6;
At2<4, 4> at2;
static_assert(alignof(At2<4, 4>) == 8);
