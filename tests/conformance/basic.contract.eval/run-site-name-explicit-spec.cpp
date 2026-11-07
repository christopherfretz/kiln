// [support.contract.violation]/4 (P2900R14), [support.srcloc.cons]: the
// violation's location().function_name() is the function the assertion
// belongs to, spelled as kiln's (clang's) __PRETTY_FUNCTION__ there. An
// explicit specialization of a member of a class template prints its
// own parameter types (`m(long)`, not the template's `m(T)`), as clang
// 22's __PRETTY_FUNCTION__ does; an implicit instantiation prints the
// pattern's. GCC 16.1 spells every name in its own style
// (`int C<T>::m(T) [with T = long int]`), so there is no oracle.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: int C<long>::m(long) [T = long] | x > 100 | same
//@ stdout: int C<int>::m(T) [T = int] | x > 0 | same
//@ stdout: int C<long>::t(int) [T = long] | u > 100 | same
//@ stdout: int C<int>::t(U) [T = int, U = int] | u > 0 | same
//@ stdout: int g(long) | x > 100 | same
//@ stdout: int g(T) [T = int] | x > 0 | same
#include <contracts>
#include <cstdio>
#include <cstring>
static const char* last = "";
void handle_contract_violation(const std::contracts::contract_violation& v) {
  last = v.location().function_name();
  std::printf("%s | %s | ", last, v.comment());
}
// Run after the violated precondition: compares with __PRETTY_FUNCTION__.
static int body(const char* pretty, int r) {
  std::printf("%s\n", std::strcmp(last, pretty) == 0 ? "same" : pretty);
  return r;
}
template <class T> struct C {
  int m(T x) pre(x > 0) { return body(__PRETTY_FUNCTION__, 1); }
  template <class U> int t(U u) pre(u > 0) { return body(__PRETTY_FUNCTION__, 1); }
};
template <> int C<long>::m(long x) pre(x > 100) { return body(__PRETTY_FUNCTION__, 3); }
template <> template <> int C<long>::t(int u) pre(u > 100) {
  return body(__PRETTY_FUNCTION__, 3);
}
template <class T> int g(T x) pre(x > 0) { return body(__PRETTY_FUNCTION__, 1); }
template <> int g(long x) pre(x > 100) { return body(__PRETTY_FUNCTION__, 3); }
int main() {
  C<long>{}.m(5L);
  C<int>{}.m(-1);
  C<long>{}.t(5);
  C<int>{}.t(-1);
  g(5L);
  g(-1);
}
