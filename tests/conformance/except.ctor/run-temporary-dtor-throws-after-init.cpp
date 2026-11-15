// [except.ctor]/2: an object whose initialization has completed is
// destroyed when a destructor of a temporary of that initialization's
// full-expression then throws, before the temporaries older than it, and
// exactly once: a variable (by constructor and by aggregate
// initialization), an array, a base and a member initialized by a
// mem-initializer, and each element of an array constructed in a loop
// (a default argument's temporary is destroyed after each element). Found
// running GCC's C++ testsuite (GCC 16 leaks the member and the base).
//@ xfail(clang): clang 19 and 22 leak the completed object
//@ expect: run
//@ oracle: none
//@ stdout: T1 B2 ~T1 ~B2 | T1 T2 B3 ~T2 ~B3 ~T1 | T1 B2 T3 B4 ~T3 ~B4 ~B2 ~T1 | T1 B2 T3 B4 ~T3 ~B4 ~B2 ~T1 | T1 B2 ~T1 T3 B4 ~T3 ~B4 ~B2 | T1 B2 ~T1 ~B2 | T1 B2 ~T1 ~B2 | c=0
extern "C" int printf(const char*, ...);
int live, next, boom;
struct T {
  int id;
  T() : id(++next) { ++live; printf("T%d ", id); }
  ~T() noexcept(false) {
    --live;
    printf("~T%d ", id);
    if (id == boom) throw 0;
  }
};
struct B {
  int id;
  B(const T& = T()) : id(++next) { ++live; printf("B%d ", id); }
  B(const T&, const T&) : id(++next) { ++live; printf("B%d ", id); }
  ~B() { --live; printf("~B%d ", id); }
};
struct P { B x, y; };
struct D : B { D() : B(T()) {} };
struct M {
  B b;
  M() : b(T()) {}
  ~M() { printf("~M "); }
};
struct Outer { M m; B c; Outer() : m(), c() {} };
template <class F> void run(int b, F f) {
  next = 0;
  boom = b;
  try { f(); } catch (int) {}
  printf("| ");
}
int main() {
  run(1, [] { B v(T{}); });                 // variable, one temporary
  run(2, [] { B v(T{}, T{}); });            // the newest temporary throws
  run(3, [] { P p = { T{}, T{} }; });       // aggregate
  run(3, [] { B a[2] = { T{}, T{} }; });    // array from a braced list
  run(3, [] { B a[2]; });                   // constructor loop
  run(1, [] { Outer o; });                  // mem-initializer of a member
  run(1, [] { D d; });                      // mem-initializer of a base
  printf("c=%d\n", live);
}
