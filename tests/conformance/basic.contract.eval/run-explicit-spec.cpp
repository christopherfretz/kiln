// [temp.expl.spec]/12, [dcl.contract.func]/3-4 (P2900R14 §3.4.7): the
// function contract assertions of an explicit specialization (of a
// function template, or of a member of a class template) are its own,
// independent of the template's: one without a seq checks nothing, one
// with a seq checks that; a seq on its first declaration only applies
// to the definition that omits it (with the definition's parameter
// names). Also in a constant evaluation. GCC 16.1 drops the seq of an
// explicit specialization's first declaration when its definition omits
// it (no "x > 0" lines), so there is no oracle (spec/contracts.md §9).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: 2 2 1
//@ stdout: handler x > 100
//@ stdout: 3 3
//@ stdout: handler x > 0
//@ stdout: handler x > 0
//@ stdout: 4 4
//@ xfail(gcc): GCC 16.1 drops the seq of an explicit specialization's first declaration when its definition omits it (spec/contracts.md §9)
#include <contracts>
#include <cstdio>
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("handler %s\n", v.comment());
}
template <class T> constexpr int g(T x) pre(x > 0) { return 1; }
template <> constexpr int g(int x) { return 2; }
template <> constexpr int g(long x) pre(x > 100) { return 3; }
template <> constexpr int g(short x) pre(x > 0);
template <> constexpr int g(short y) { return 4; }
template <class T> struct C { int m(T x) pre(x > 0) { return 1; } };
template <> int C<int>::m(int x) { return 2; }
template <> int C<long>::m(long x) pre(x > 100) { return 3; }
template <> int C<short>::m(short x) pre(x > 0);
template <> int C<short>::m(short y) { return 4; }
static_assert(g(-1) == 2 && g(101L) == 3 && g(short(1)) == 4);
int main() {
  std::printf("%d %d %d\n", g(-1), C<int>{}.m(-1), g(1.0));
  int a = C<long>{}.m(5L);
  std::printf("%d %d\n", a, g(101L));
  int b = g(short(-1));
  std::printf("%d %d\n", b, C<short>{}.m(-1));
}
