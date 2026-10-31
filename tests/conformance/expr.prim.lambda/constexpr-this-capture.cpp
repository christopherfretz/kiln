// [expr.prim.lambda.capture]/10, [expr.prim.this]: `this` captured by
// reference and `*this` by copy in constant evaluation.
struct S {
  int v;
  constexpr int get() const { return [this] { return v; }(); }
  constexpr int bump() { [this] { ++v; }(); return v; }
  constexpr int star() { auto l = [*this]() mutable { return ++v; }; int r = l(); return r * 10 + v; }
  constexpr int impl() const { return [&] { return v + 1; }(); }
  constexpr int call() const { return [this] { return get() * 2; }(); }
  constexpr int nested() const { return [this] { return [this] { return v; }(); }(); }
};
static_assert(S{4}.get() == 4);
static_assert(S{4}.bump() == 5);
static_assert(S{4}.star() == 54);
static_assert(S{4}.impl() == 5);
static_assert(S{4}.call() == 8);
static_assert(S{4}.nested() == 4);
