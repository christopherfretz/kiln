//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1 1 1 1 1 1
//@ stdout: 1 1 8 8
//@ diag: 18:61 warning "'mode' attribute ignored when parsing type"
//@ diag: 19:37 warning "'mode' attribute ignored when parsing type"
//@ diag: 20:32 warning "'mode' attribute ignored when parsing type"
//@ diag: 22:38 warning "'mode' attribute ignored when parsing type"
//@ diag: 23:33 warning "'mode' attribute ignored when parsing type"
// GCC mode is a declaration attribute (WP-809): in a type-id that declares
// nothing it is ignored, with clang's -Wignored-attributes warning where
// clang `ActOnTypeName` checks for unused declaration attributes (not in a
// named cast, a new-expression or an alias-declaration's type-id). The
// alias's own attributes (after its name) still apply.
#include <cstdio>

template <class T> struct Z { static constexpr int n = sizeof(T); };
template <class T> int k() { return sizeof(T __attribute__((mode(DI)))); }
int a1 = sizeof(char __attribute__((mode(DI))));
int a2 = Z<char __attribute__((mode(DI)))>::n;
int a3 = sizeof(static_cast<char __attribute__((mode(DI)))>(0));
int a4 = sizeof((char __attribute__((mode(DI))))0);
int a5 = alignof(__attribute__((mode(DI))) char);
using A = char __attribute__((mode(DI)));
template <class T> struct S {
  using V = T __attribute__((mode(DI)));
  using Y __attribute__((mode(DI))) = T;
  typedef T W __attribute__((mode(DI)));
};

int main() {
  auto *np = new char __attribute__((mode(DI)));
  int a6 = sizeof(*np);
  delete np;
  std::printf("%d %d %d %d %d %d %d %d\n", k<char>(), a1, a2, a3, a4, a5, a6, (int)sizeof(A));
  std::printf("%d %d %d %d\n", (int)sizeof(S<char>::V), (int)sizeof(char), (int)sizeof(S<char>::Y),
              (int)sizeof(S<char>::W));
}
