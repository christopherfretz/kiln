// [expr.prim.lambda.closure]/5 (C++23, P0847R7): at run time, the call
// operator of a lambda with an explicit object parameter reaches its
// captures through that parameter, also when it is of a class derived
// from the closure type, and `this` is the captured one
// ([expr.prim.this]/2). (Constant evaluation:
// p0847-lambda-explicit-object.cpp; these cases are run-time only since
// clang 19 and 22 mis-evaluate them as constants.)
//@ expect: run
//@ std: c++23
//@ strict-diags
int derived(int v) {
  auto l = [v](this auto const& self) { return v + self.extra; };
  struct D : decltype(l) { int extra; };
  D d{l, 5};
  return d();
}

struct S {
  int v = 3;
  int get() {
    auto l = [this](this auto self) { return v + this->v; };
    return l();
  }
};

int fib(int n) {
  auto f = [](this auto const& self, int k) -> int {
    return k < 2 ? k : self(k - 1) + self(k - 2);
  };
  return f(n);
}

int main() {
  if (derived(7) != 12) return 1;
  if (S{}.get() != 6) return 2;
  if (fib(12) != 144) return 3;
  int x = 1, y = 0;
  auto inc = [&y, x](this auto& self) { y += x; return y; };
  inc();
  if (inc() != 2) return 4;
  return 0;
}
