// [expr.prim.lambda.general], [dcl.fct.default]: a lambda may appear in a
// default argument and a default member initializer, and may have default
// arguments of its own.
constexpr int call(int (*f)(int) = [](int x) { return x + 1; }, int v = 1) { return f(v); }
static_assert(call() == 2);
static_assert(call([](int x) { return x * 5; }, 2) == 10);
struct S {
  int (*f)(int) = [](int x) { return x * 2; };
  constexpr int g(int (*h)() = [] { return 7; }) const { return h(); }
};
static_assert(S{}.f(4) == 8);
static_assert(S{}.g() == 7);
constexpr auto d = [](int a, int b = 10) { return a + b; };
static_assert(d(1) == 11);
static_assert(d(1, 2) == 3);
