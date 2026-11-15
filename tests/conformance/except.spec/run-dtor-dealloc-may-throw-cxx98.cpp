// C++03 [except.spec]/13: only an implicitly declared special member gets
// an implicit exception specification; a destructor or a class-specific
// operator delete declared without one may throw anything (C++03 [class.dtor]
// has no implicit noexcept; that came with C++11 [class.dtor]/3 and
// [except.spec]/15). An implicit destructor that calls such a destructor
// allows all exceptions too. kiln gave all three the C++11 noexcept and
// called terminate (found running GCC's C++ testsuite). A destructor
// defined out of class without a specification redeclares the in-class one
// (kiln reported a mismatch against the C++11 implicit one); so does a
// class template's.
//@ expect: run
//@ std: c++98
//@ args: -w
//@ stdout: caught 1
//@ stdout: caught 2
//@ stdout: caught 3
//@ stdout: caught 4
//@ stdout: caught 5
#include <cstdio>
#include <cstdlib>
#include <exception>
struct B { int n; B(int n) : n(n) {} ~B() { throw n; } };
struct C { B b; C() : b(2) {} };
struct D { void operator delete(void* p) { std::free(p); throw 3; } };
struct E { inline ~E(); };
inline E::~E() { throw 4; }
template <class T> struct F { ~F(); };
template <class T> F<T>::~F() { throw T(5); }
void* operator new(std::size_t n, int) { return std::malloc(n); }
static void term() { std::printf("terminate\n"); std::exit(1); }
int main() {
  std::set_terminate(term);
  try { B b(1); } catch (int i) { std::printf("caught %d\n", i); }
  try { C c; } catch (int i) { std::printf("caught %d\n", i); }
  try { delete new (0) D; } catch (int i) { std::printf("caught %d\n", i); }
  try { E e; } catch (int i) { std::printf("caught %d\n", i); }
  try { F<int> f; } catch (int i) { std::printf("caught %d\n", i); }
}
