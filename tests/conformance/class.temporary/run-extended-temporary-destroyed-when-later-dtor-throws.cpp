// [except.ctor]/2, [class.temporary]/6: a temporary bound to a reference
// is complete once its full-expression's other temporaries are destroyed;
// when one of those destructors throws, the extended temporary (a class
// object, an array of them, an aggregate) is destroyed too, exactly once.
// Found running GCC's C++ testsuite.
//@ xfail(clang): clang 19 and 22 leak the extended temporary
//@ expect: run
//@ oracle: none
//@ stdout: 0 0 0
extern "C" int printf(const char*, ...);
int live;
struct Q {
  int q = 0;
  ~Q() noexcept(false) { throw 0; }
};
struct A {
  A() { ++live; }
  A(int) { ++live; }
  A(const Q&) { ++live; }
  A(const A&) = delete;
  ~A() { --live; }
};
using Arr = A[3];
struct Agg { A a; const A& r; };
int main() {
  try { const A& a = Q().q; (void)a; } catch (int) {}
  printf("%d ", live);
  live = 0;
  try { auto&& r = Arr{Q()}; (void)r; } catch (int) {}
  printf("%d ", live);
  live = 0;
  try { const Agg& g = Agg{A(), Q()}; (void)g; } catch (int) {}
  printf("%d\n", live);
}
