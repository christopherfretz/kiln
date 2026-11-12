// [basic.types.general]/12, [expr.const]/25.3 (P2996R13): two neighbours
// of the expansion-variable case (stmt.expand/consteval-only-element-neg.cpp),
// rejected at run time like a plain `constexpr E e`: a `constexpr T x` of a
// function template instantiated with a class that has a std::meta::info
// member (the pattern's `x` has a dependent type; checked once, at the
// instantiation of line 30, and not for `tq<int>`), and a binding of a
// `constexpr` structured binding of such a class (`n` is a `const char*`
// but names a subobject of a consteval-only object). Before WP-1330 kiln
// accepted both. GCC 16.1 rejects both.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 24:67 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 30:3 note "in instantiation of function template specialization 'tp<E>' requested here"
//@ diag: 26:56 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 27:81 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 32:3 note "in instantiation of function template specialization 'tb<E>' requested here"
#include <meta>
#include <cstdio>
struct E { std::meta::info r; const char* name; };
constexpr E plan[] = {{^^int, "int"}, {^^long, "long"}};
template <class T> void tp() { constexpr T x = plan[0]; std::puts(x.name); }
template <class T> void tq() { constexpr T x = 1; std::printf("%d", x); }
void sb() { constexpr auto [r, n] = plan[0]; std::puts(n); }
template <class T> void tb() { constexpr auto [r, n] = T{^^int, "b"}; std::puts(n); }
int main() {
  tq<int>();
  tp<E>();
  sb();
  tb<E>();
}
