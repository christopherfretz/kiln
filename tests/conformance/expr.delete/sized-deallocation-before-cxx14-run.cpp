// [expr.delete]/10 (C++14, N3778): sized deallocation starts in C++14. Before it, a global
// `operator delete(void*, std::size_t)` is a placement deallocation function that a
// delete-expression never calls, for a scalar, a class or an array with a cookie. Found running
// GCC's C++ testsuite (C++98 only).
//@ expect: run
//@ args: -std=c++11
//@ stdout: 000
#include <cstdio>
#include <new>
bool called;
void operator delete(void* p, std::size_t) throw() { called = true; operator delete(p); }
void operator delete[](void* p, std::size_t) throw() { called = true; operator delete[](p); }
struct A { ~A() {} };
int main() {
  delete new int;
  std::printf("%d", called);
  called = false;
  delete new A;
  std::printf("%d", called);
  called = false;
  delete[] new A[2];
  std::printf("%d\n", called);
}
