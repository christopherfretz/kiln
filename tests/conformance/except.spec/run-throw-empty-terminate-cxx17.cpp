// C++17 [except.spec]/1 (P0003R5): `throw()` is `noexcept(true)`, so a
// throw out of it calls terminate directly ([except.terminate]); before
// C++17 it is a dynamic exception specification that calls the unexpected
// handler first (run-dynamic-spec-unexpected-cxx14.cpp).
//@ expect: run
//@ std: c++17
//@ args: -w
//@ stdout: terminate
#include <cstdio>
#include <cstdlib>
#include <exception>
static void term() { std::printf("terminate\n"); std::exit(0); }
void f() throw() { throw 1; }
int main() {
  std::set_terminate(term);
  try { f(); } catch (...) { std::printf("caught\n"); }
  return 1;
}
