// [expr.prim.lambda.capture]/3.3, [basic.pre]/9, [basic.contract.eval]
// (P2900R14): at run time a lambda in a precondition or postcondition
// captures the function's parameters and the result binding, by copy and
// by reference (a reference capture of the result binding of a reference
// return names the returned object); under enforce a predicate false
// through a capture calls the handler and terminates. GCC 16.1 rejects
// naming a parameter in such a lambda, so there is no oracle
// (spec/contracts.md §9).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=enforce
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ exit: 7
//@ stdout: 3 8 7 1 2
//@ stdout: 1
//@ stdout: handler 1
//@ stdout: terminate
//@ xfail(gcc): GCC 16.1 rejects naming a parameter in a lambda in a predicate (spec/contracts.md §9)
#include <contracts>
#include <cstdio>
#include <cstdlib>
#include <exception>
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("handler %d\n", int(v.kind()));
}
template <class T> T tf(const T a, const T& b)
  pre([a, &b] { return a < b; }()) post(r: [=] { return r == a + b; }()) { return a + b; }
int decl(const int a) pre([&] { return a > 0; }()) post(r: [r, a] { return r == a * 2; }());
int decl(const int x) { return x * 2; }
struct S {
  int m = 2;
  int h(const int k) const pre([this, k] { return m < k; }())
    post(r: [&] { return r == k + m; }()) { return k + m; }
};
int mut(int a) pre([=]() mutable { ++a; return a > 1; }()) { return a; }
int& refret(int& x) post(r: [&r, &x] { return &r == &x; }()) { return x; }
auto gl = [](const auto a) pre([a] { return a > 0; }()) { return a; };
int gx;
int main() {
  std::set_terminate([] {
    std::printf("terminate\n");
    std::fflush(stdout);
    std::_Exit(7);
  });
  std::printf("%d %d %d %d %d\n", tf(1, 2), decl(4), S{}.h(5), mut(1), gl(2));
  std::printf("%d\n", &refret(gx) == &gx);
  std::fflush(stdout);
  return decl(-1);
}
