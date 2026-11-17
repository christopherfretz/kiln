// [expr.delete]/7 (C++26, P3068R6): the deallocation function is called
// even when the destructor exits by an exception, which then propagates.
// For `delete[]` the elements are destroyed in decreasing address order
// and, when one destructor throws, the remaining elements are still
// destroyed ([except.ctor]/2; GCC 16.1 agrees, at run time too); the
// members of an object whose destructor body throws are destroyed
// ([except.ctor]/4); through a base with a virtual destructor the
// most-derived object is released. 10000 rounds of a throwing `delete[]`
// keep a flat heap. Found running GCC's C++ testsuite (kiln reported the
// storage as never deallocated).
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ oven: 66 allocs<=20000 peak_bytes<=64 peak_arena<=512
//@ mem<=512
struct T {
  int *log;
  constexpr ~T() noexcept(false) {
    if (*log == 1) throw 42;
    ++*log;
  }
};
constexpr int scalar() {
  int log = 1;
  T *p = new T{&log};
  try { delete p; } catch (int e) { return e; }
  return 0;
}
static_assert(scalar() == 42);

struct U {
  int *log, i;
  constexpr ~U() noexcept(false) {
    if (i == 1) throw i;
    *log = *log * 10 + i;
  }
};
constexpr int array() {
  int log = 0;
  U *p = new U[4]{{&log, 3}, {&log, 0}, {&log, 1}, {&log, 2}};
  try { delete[] p; } catch (int e) { return log * 10 + e; }
  return -1;
}
static_assert(array() == 2031);

struct Own { int *q; constexpr ~Own() { delete q; } };
struct M {
  Own o;
  int k;
  constexpr ~M() noexcept(false) { throw k; }
};
constexpr int members() {
  M *p = new M{{new int(5)}, 3};
  try { delete p; } catch (int e) { return e; }
  return 0;
}
static_assert(members() == 3);

struct E { int i; constexpr ~E() noexcept(false) { if (i) throw i; } };
constexpr int churn(int rounds) {
  int n = 0;
  for (int i = 0; i < rounds; ++i) {
    E *p = new E[3]{{0}, {1}, {0}};
    try { delete[] p; } catch (int e) { n += e; }
  }
  return n;
}
static_assert(churn(10000) == 10000);

struct B { constexpr virtual ~B() noexcept(false) {} };
struct D : B {
  int *log;
  long long pad[4] = {};
  constexpr explicit D(int *l) : log(l) {}
  constexpr ~D() noexcept(false) override {
    ++*log;
    throw 9;
  }
};
constexpr int virt() {
  int log = 0;
  B *p = new D(&log);
  try { delete p; } catch (int e) { return e * 10 + log; }
  return 0;
}
static_assert(virt() == 91);
