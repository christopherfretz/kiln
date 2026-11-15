// C++11 [class.dtor]/3 and [except.spec]/14-15: from C++11 a destructor
// without a noexcept-specifier gets the implicit specification (here
// noexcept(true)), and so does a class-specific deallocation function; a
// throw that leaves them calls terminate ([except.terminate]). The C++98
// counterpart is run-dtor-dealloc-may-throw-cxx98.cpp.
//@ expect: run
//@ std: c++11
//@ args: -w
//@ stdout: terminate
#include <cstdio>
#include <cstdlib>
#include <exception>
struct B { int n; B(int n) : n(n) {} ~B() { throw n; } };
struct C { B b; C() : b(2) {} };
struct D { void operator delete(void* p) { std::free(p); throw 3; } };
static_assert(noexcept(static_cast<B*>(0)->~B()), "");
static_assert(noexcept(static_cast<C*>(0)->~C()), "");
static_assert(noexcept(D::operator delete(0)), "");
static void term() { std::printf("terminate\n"); std::exit(0); }
int main() {
  std::set_terminate(term);
  try { B b(1); } catch (int i) { std::printf("caught %d\n", i); }
  return 1;
}
