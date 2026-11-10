// [expr.prim.lambda.closure]/3, [dcl.spec.auto]: generic lambdas, a nested
// generic lambda and a `decltype(auto)` trailing return type, instantiated
// with their enclosing templates at C++14 (the C++11 deduced-return gate
// exempts the instantiated call operator; WP-1213).
//@ expect: run
//@ std: c++14
//@ args: -pedantic
//@ strict-diags
//@ stdout: 23|105|9
extern "C" int printf(const char *, ...);

template <class F> int take(F f) { return f(2, 3); }

template <class S> int generic() {
  return take([](auto a, S b) {
    auto inner = [](auto x) { return x * 10; };
    return inner(a) + b;
  });
}

template <class S> struct Box {
  S v;
  int get() const {
    return take([this](auto a, auto b) { return v + a + b; });
  }
};

template <class S> decltype(auto) ref_of(S &s) {
  return [&s]() -> decltype(auto) { return (s); }();
}

int main() {
  int x = 4;
  ref_of(x) = 9;
  printf("%d|%d|%d\n", generic<int>(), Box<int>{100}.get(), x);
}
