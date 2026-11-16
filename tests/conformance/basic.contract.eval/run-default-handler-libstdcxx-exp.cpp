// [basic.contract.handler]/3 (P2900R14), spec/driver.md "Link options"
// (WP-1485): at a plain -std=c++26, with GCC 16.1's libstdc++ (whose
// default contract-violation handler is in libstdc++exp.a), kiln++
// links libstdc++exp without -fcontracts, so a program with a failing
// precondition and no handler of its own links, and under enforce the
// default handler runs and std::terminate is called. g++ 16.1 does not
// link the library here (the same program needs -fcontracts there), so
// there is no oracle.
//@ expect: run
//@ args: --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: terminate
#include <contracts>
#include <cstdio>
#include <cstdlib>
#include <exception>
int f(int x) pre(x > 0) { return x; }
int main() {
  std::set_terminate([] {
    std::puts("terminate");
    std::fflush(stdout);
    std::_Exit(0);
  });
  f(0);
  std::puts("not terminated");
  return 1;
}
