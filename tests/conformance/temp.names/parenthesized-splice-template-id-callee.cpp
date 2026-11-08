// P2996R13 [expr.prim.splice]/3 [temp.arg.explicit]/4 [over.call.func]:
// a parenthesized splice template-id that is the whole of a call's callee,
// `(template [: r :]<A>)(x, y)`, stays an overload set whose remaining
// template arguments the call deduces, as `(f<A>)(x, y)` does; not
// called, it names its single specialization ([over.over]/3).
// spec/sema-overload.md §2 (called template-ids), WP-699 (WP-823).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: run
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 8 4 8 1
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
extern "C" int printf(const char*, ...);

template <class A, class B> int sz(A, B) { return sizeof(B); }
template <class A> int one(A) { return sizeof(A); }

int main() {
  constexpr auto r = ^^sz;
  int a = (template [: r :]<char>)('c', 2.0);
  int b = (template [: ^^sz :]<char>)('c', 2.0f);
  int c = ((template [: r :]<char>))('c', 2.0);
  int (*p)(char) = (template [: ^^one :]<char>);
  printf("%d %d %d %d\n", a, b, c, p('x'));
}
