// Shared body of the run-dynamic-spec-unexpected-* tests (C++98 to C++14
// [except.spec] and [except.unexpected]): a function that throws a type
// its dynamic exception specification does not allow calls the unexpected
// handler; what the handler throws propagates when the specification
// allows it, becomes std::bad_exception when the specification lists
// that, and otherwise calls terminate.
#include <cstdio>
#include <cstdlib>
#include <exception>
struct A {};
struct B : A {};
static void unexp() { std::printf("U "); throw 42; }
static void term() { std::printf("terminate\n"); std::exit(0); }
void f1() throw(int) { throw 'c'; }
void f2() throw(A) { throw B(); }
void f3() throw(const A*) { static B b; throw &b; }
void f4() throw(short, std::bad_exception) { throw 'c'; }
void f5() throw(int) try { throw 1.0; } catch (double) { throw 'c'; }
struct V { virtual void g() throw(int) { throw 'c'; } virtual ~V() {} };
struct W : V { void g() throw(int) { throw 1.5f; } };
void f7() throw(char, int) { throw 2.0; }
template <class T> void f8(T) throw(T) { throw 'c'; }
struct K { K() throw(int) { throw 'c'; } };
struct L { ~L() throw(int) { throw 'c'; } };
struct D { ~D() { std::printf("~D "); } };
void f9() throw(int) { D d; throw 'c'; }
void f10() throw() { throw 1; }
int main() {
  std::set_unexpected(unexp);
  std::set_terminate(term);
  try { f1(); } catch (int i) { std::printf("f1 %d\n", i); }
  try { f2(); } catch (A&) { std::printf("f2 A\n"); }
  try { f3(); } catch (const A*) { std::printf("f3 ptr\n"); }
  try { f4(); } catch (std::bad_exception&) { std::printf("f4 bad\n"); }
  try { f5(); } catch (int i) { std::printf("f5 %d\n", i); }
  W w;
  V& v = w;
  try { v.g(); } catch (int i) { std::printf("f6 %d\n", i); }
  void (*fp)() throw(char, int) = f7;
  try { fp(); } catch (int i) { std::printf("f7 %d\n", i); }
  try { f8(1); } catch (int i) { std::printf("f8 %d\n", i); }
  try { K k; } catch (int i) { std::printf("ctor %d\n", i); }
  try { L l; } catch (int i) { std::printf("dtor %d\n", i); }
  try { f9(); } catch (int i) { std::printf("f9 %d\n", i); }
  f10();
  std::printf("not reached\n");
  return 1;
}
