// [expr.const.const]/3, [expr.const.imm]/2.1 (P4101R1): `b` (a `constexpr`
// pointer to an immediate function) is an immediate object, so naming it,
// calling through it or copying it outside an immediate function context
// is an error, in the non-dependent spelling and once instantiated
// (function template, class template member, lambda in a template, a
// dependent callee). A non-`constexpr` variable may not hold the pointer.
// An immediate-escalating function escalates instead (`r3`, `u6`'s
// lambda). A default argument naming an immediate object is reported at
// the call that uses it. kiln only: GCC 16.1 rejects `b` itself
// (docs/status/deviations.md).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P4101R1/P3603
//@ strict-diags
//@ diag: 44:19 error "consteval-only value outside an immediate function context"
//@ diag: 47:24 error "call to immediate function 'r3<int>' is not a constant expression"
//@ diag: 46:51 note "'r3<int>' is an immediate function because its body contains a consteval-only value of type 'int (*const)(int)'"
//@ diag: 47:27 note "function parameter 'x' with unknown value cannot be used in a constant expression"
//@ diag: 47:12 note "declared here"
//@ diag: 48:10 error "cannot take address of consteval function 'plus1' outside of an immediate invocation"
//@ diag: 41:15 note "declared here"
//@ diag: 53:3 error "'h2' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 57:24 error "call to immediate function 'u7(int)::("
//@ diag: 57:25 note "function parameter 'x' with unknown value cannot be used in a constant expression"
//@ diag: 57:12 note "declared here"
//@ diag: 69:14 error "consteval-only value outside an immediate function context"
//@ diag: 42:49 error "consteval-only value outside an immediate function context"
//@ diag: 45:30 note "in instantiation of member function 'C<&plus1>::get' requested here"
//@ diag: 49:65 error "consteval-only value outside an immediate function context"
//@ diag: 50:10 note "in instantiation of function template specialization 'r4<int>' requested here"
//@ diag: 54:49 error "consteval-only value outside an immediate function context"
//@ diag: 55:20 note "in instantiation of member function 'TM<int>::f' requested here"
//@ diag: 58:38 error "consteval-only value outside an immediate function context"
//@ diag: 59:10 note "in instantiation of function template specialization 'r8<int>' requested here"
//@ diag: 60:40 error "consteval-only value outside an immediate function context"
//@ diag: 61:10 note "in instantiation of function template specialization 'r9<int>' requested here"
//@ diag: 62:39 error "consteval-only value outside an immediate function context"
//@ diag: 63:10 note "in instantiation of function template specialization 'a3<int>' requested here"
//@ diag: 64:74 error "consteval-only value outside an immediate function context"
//@ diag: 65:18 note "in instantiation of function template specialization 'O<int>::f<int>' requested here"
consteval int plus1(int x) { return x + 1; }
template <auto V> struct C { int get() { return V(1); } };
constexpr auto b = plus1;
int r1() { return b(1); }
int r2() { return C<plus1>{}.get(); }
template <class T> constexpr int r3(T t) { return b(t); }
int u3(int x) { return r3(x); }
auto a = plus1;
template <class T> int r4() { constexpr auto bb = plus1; return bb(T(1)); }
int u4 = r4<int>();
struct H { int (*p)(int); };
constexpr H h{plus1};
H h2 = h;
template <class T> struct TM { int f() { return b(T(1)); } };
int u5 = TM<int>{}.f();
int u6 = [] { return b(1); }();
int u7(int x) { return [=] { return b(x); }(); }
template <class T> int r8() { return b(1) + T(0); }
int u8 = r8<int>();
template <class T> int r9() { auto p = b; return T(0); }
int u9 = r9<int>();
template <class T> int a3() { return (b)(T(1)); }
int v3 = a3<int>();
template <class T> struct O { template <class U> static int f() { return b(U(4)); } };
int v4 = O<int>::f<int>();
using info = decltype(^^int);
constexpr info cr = ^^int;
void hd(const info* p = &cr) {}
void use() { hd(); }
