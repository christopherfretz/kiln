// [expr.prim.lambda.closure]/5, [dcl.fct]/6 (C++23, P0847R7): a lambda's
// first parameter may be an explicit object parameter. Its call operator
// is then not `const`; captures are reached through the object parameter
// (which may be a class derived from the closure type), a capture by
// copy is `const` exactly when the object parameter's type is
// ([expr.prim.id.unqual]/3), and `this` is the enclosing class's
// ([expr.prim.this]/2). A captureless one converts to a pointer to the
// call operator ([expr.prim.lambda.closure]/8). (Run-time
// behaviour: p0847-lambda-explicit-object-run.cpp.)
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

constexpr int fib(int n) {
  auto f = [](this auto const& self, int k) -> int {
    return k < 2 ? k : self(k - 1) + self(k - 2);
  };
  return f(n);
}
static_assert(fib(10) == 55);

constexpr int captures() {
  int base = 100, total = 0;
  auto by_copy = [base](this auto self, int n) { return base + n; };
  auto by_ref = [&total](this auto&& self, int n) { total += n; return total; };
  by_ref(2);
  return by_copy(1) + by_ref(3);
}
static_assert(captures() == 106);

// A copy captured into a non-const object parameter is modifiable: the
// object parameter is a copy of the closure, so the original keeps its value.
constexpr int counter() {
  int n = 0;
  auto next = [n](this auto self) { return ++n; };
  next();
  return next() + n;
}
static_assert(counter() == 1);

// Type of the object parameter and of a capture by copy.
void types() {
  int x = 0;
  auto a = [x](this auto& self) {
    static_assert(same<decltype(self), decltype(self)&>);
    return same<decltype((x)), int&>;
  };
  auto b = [x](this const auto& self) { return same<decltype((x)), const int&>; };
  auto c = [x](this auto self) { return same<decltype((x)), int&>; };
  (void)a; (void)b; (void)c;
}

struct S {
  int v = 3;
  constexpr int get() {
    auto l = [this](this auto self) { return v + this->v; };
    return l();
  }
};
static_assert(S{}.get() == 6);

// A captureless one converts to a pointer to its call operator, whose
// first parameter is the object parameter.
constexpr int ptr() {
  auto m = [](this auto, int n) { return n * 2; };
  int (*p)(decltype(m), int) = m;
  return p(m, 4);
}
static_assert(ptr() == 8);

