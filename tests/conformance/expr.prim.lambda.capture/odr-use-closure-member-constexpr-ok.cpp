// [expr.prim.lambda.capture]/11 and [expr.const]/10, accepted side: a name
// of a captured constant that is not an odr-use still names the variable
// and reads its value ([basic.def.odr]/5); a by-reference capture names the
// variable itself; naming the closure member without reading it
// (`sizeof`, a member function that reads nothing) is constant; and in a
// call of the call operator evaluated as part of the constant expression
// the closure object exists, so its members are read (a `mutable` lambda
// modifies its copy, not the variable), a constant closure's member has a
// constant address, and `this` and `*this` captures, generic and nested
// lambdas and a lambda in a default member initializer work. A case value
// that names a captured constant reads the variable. Found running GCC's
// C++ testsuite.
//@ expect: compile-ok
//@ std: c++26
//@ requires: clang>=22
struct P { int x, y; };
struct S {
  int v;
  constexpr int get() const { return 7; }
  constexpr int val() const { return v; }
};
constexpr int rd(const int &r) { return r; }

void g() {
  const int n = 1;
  constexpr int x = 2;
  constexpr S s{3};
  constexpr auto [a, b] = P{4, 5};
  auto l1 = [=] { constexpr int i = n + x + s.v; return i + n; };
  auto l2 = [n] { constexpr int i = n; return i + n; };
  auto l3 = [&] { constexpr int j = *&n + rd(x) + s.val(); return j; };
  auto l4 = [=] { constexpr int j = s.get(); return j + s.v; };
  auto l5 = [=] { constexpr unsigned long z = sizeof(&n); return z + n; };
  auto l6 = [&] { constexpr int j = *&b; return j; };
  auto l7 = [=](int k) {
    switch (k) {
    case n: return n;
    case x + n: return x;
    }
    return 0;
  };
  auto l8 = [=](auto k) {
    switch (k) {
    case n: return n;
    }
    return 0;
  };
  (void)l1; (void)l2; (void)l3; (void)l4; (void)l5; (void)l6; (void)l7; (void)l8(1); (void)a;
  // A constant closure: the address of its member is constant.
  static constexpr auto c = [=] { return &n; };
  constexpr const int *p = c();
  static_assert(*p == 1 && p == c());
}

constexpr int mut() {
  int n = 1;
  auto l = [=]() mutable { return ++n; };
  int a = l();
  int b = l();
  return n * 100 + a * 10 + b;
}
static_assert(mut() == 123);

constexpr int nested() {
  int n = 5;
  auto outer = [=]() mutable {
    ++n;
    auto inner = [&] { return *&n * 10; };
    auto copy = [=] { return n; };
    return inner() + copy();
  };
  return outer() * 1000 + n;
}
static_assert(nested() == 66005);

constexpr int generic() {
  const int n = 4;
  auto l = [=](auto k) { return *&n + k; };
  return l(1) + l(2L);
}
static_assert(generic() == 11);

constexpr int reads() {
  S s{8};
  const int n = 3;
  auto l = [=] {
    constexpr int i = n;
    return s.val() + rd(n) + i + *&n;
  };
  return l();
}
static_assert(reads() == 17);

constexpr int binding() {
  auto [a, b] = P{1, 2};
  auto l = [=] { return *&a * 10 + b; };
  return l();
}
static_assert(binding() == 12);

struct T {
  int v = 2;
  constexpr int f() const {
    return [*this] { return v; }() + [this] { return *&v; }();
  }
};
static_assert(T{}.f() == 4);

struct U {
  int a = 3;
  int b = [this] { return a * 2; }();
};
static_assert(U{}.b == 6);
