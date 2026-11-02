// [except.handle]/13-15 in constant evaluation (C++26, P3068R6): a
// function-try-block's handler may return from an ordinary function, and
// flowing off the end of a constructor's or destructor's handler rethrows
// the handled exception; flowing off the end of a value-returning
// function's handler is not a constant expression ([stmt.return]/4).
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 49:1 warning "non-void function does not return a value in all control paths"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:1 note "control reached end of constexpr function"
//@ diag: 51:15 note "in call to 'falls_off(0)'"
struct E { int v; };

struct C {
  int x;
  constexpr C(int v, int *seen) try : x(v) {
    if (v > 1) throw E{v};
  } catch (const E &e) {
    *seen = e.v;
  }
};

constexpr int construct(int v) {
  int seen = 0;
  try {
    C c(v, &seen);
    return c.x;
  } catch (E e) {
    return 100 * seen + e.v;
  }
}
static_assert(construct(1) == 1);
static_assert(construct(5) == 505);

constexpr int ordinary(int v) try {
  if (v) throw v;
  return 0;
} catch (int i) {
  return i + 1;
}
static_assert(ordinary(0) == 0);
static_assert(ordinary(6) == 7);

constexpr int falls_off(int v) try {
  throw v;
} catch (int i) {
  if (i) return i;
}
static_assert(falls_off(1) == 1);
static_assert(falls_off(0) == 0);
