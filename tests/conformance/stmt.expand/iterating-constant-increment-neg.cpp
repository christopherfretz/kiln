// [stmt.expand]/5.2: N of an iterating expansion statement is computed by
// a consteval lambda that runs `for (auto b = begin-expr; b != e; ++b)`,
// so begin-expr, end-expr, `!=` and `++` must each be usable in a constant
// expression, whether or not the for-range-declaration is `constexpr`: a
// non-constexpr one is "expansion size is not a constant expression" with
// the evaluator's notes (with `constexpr`, the failed initialization of
// the implicit `__begin` or `__end` is reported first). `++` is not
// evaluated for an empty range (the accepted side:
// `iterating-constant-increment-run.cpp`). Found running GCC's C++
// testsuite (a non-constexpr `++` was accepted; GCC 16.1 rejects each).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 66:33 error "expansion size is not a constant expression"
//@ diag: 66:33 note "non-constexpr function 'operator++' cannot be used in a constant expression"
//@ diag: 46:7 note "declared here"
//@ diag: 67:23 error "expansion size is not a constant expression"
//@ diag: 67:23 note "non-constexpr function 'operator++' cannot be used in a constant expression"
//@ diag: 46:7 note "declared here"
//@ diag: 68:33 error "expansion size is not a constant expression"
//@ diag: 68:33 note "non-constexpr function 'operator!=' cannot be used in a constant expression"
//@ diag: 53:8 note "declared here"
//@ diag: 69:23 error "expansion size is not a constant expression"
//@ diag: 69:23 note "non-constexpr function 'operator!=' cannot be used in a constant expression"
//@ diag: 53:8 note "declared here"
//@ diag: 70:33 error "constexpr variable '__begin' must be initialized by a constant expression"
//@ diag: 70:33 note "non-constexpr function 'begin' cannot be used in a constant expression"
//@ diag: 58:25 note "declared here"
//@ diag: 71:23 error "expansion size is not a constant expression"
//@ diag: 71:23 note "non-constexpr function 'begin' cannot be used in a constant expression"
//@ diag: 58:25 note "declared here"
//@ diag: 72:33 error "constexpr variable '__end' must be initialized by a constant expression"
//@ diag: 72:33 note "non-constexpr function 'end' cannot be used in a constant expression"
//@ diag: 59:74 note "declared here"
//@ diag: 73:23 error "expansion size is not a constant expression"
//@ diag: 73:23 note "non-constexpr function 'end' cannot be used in a constant expression"
//@ diag: 59:74 note "declared here"
constexpr int a[] = {3, 4, 5};
struct It {
  const int *p;
  constexpr int operator*() const { return *p; }
  constexpr It operator+(long n) const { return {p + n}; }
  constexpr long operator-(It o) const { return p - o.p; }
  constexpr bool operator!=(It o) const { return p != o.p; }
  It &operator++() { ++p; return *this; }
};
struct Jt {
  const int *p;
  constexpr int operator*() const { return *p; }
  constexpr Jt operator+(long n) const { return {p + n}; }
  constexpr long operator-(Jt o) const { return p - o.p; }
  bool operator!=(Jt o) const { return p != o.p; }
  constexpr Jt &operator++() { ++p; return *this; }
};
struct Inc { constexpr It begin() const { return {a}; } constexpr It end() const { return {a + 3}; } };
struct Ne { constexpr Jt begin() const { return {a}; } constexpr Jt end() const { return {a + 3}; } };
struct Beg { const int *begin() const { return a; } constexpr const int *end() const { return a + 3; } };
struct End { constexpr const int *begin() const { return a; } const int *end() const { return a + 3; } };
int f() {
  static constexpr Inc inc;
  static constexpr Ne ne;
  static constexpr Beg beg;
  static constexpr End end;
  int s = 0;
  template for (constexpr int x : inc) s += x;
  template for (int x : inc) s += x;
  template for (constexpr int x : ne) s += x;
  template for (int x : ne) s += x;
  template for (constexpr int x : beg) s += x;
  template for (int x : beg) s += x;
  template for (constexpr int x : end) s += x;
  template for (int x : end) s += x;
  return s;
}
