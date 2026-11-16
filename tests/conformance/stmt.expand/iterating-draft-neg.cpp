// [stmt.expand]/5.2 (working draft): N is computed by a consteval lambda
// from begin-expr and end-expr, and each element's iterator is
// `begin + decltype(begin - begin){i}`, so a non-constexpr `begin`, an
// iterator without `-`, or a constexpr for-range-declaration over a
// non-constexpr object is ill-formed; the first error stops the statement.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 51:24 error "expansion size is not a constant expression"
//@ diag: 51:24 note "non-constexpr function 'begin' cannot be used in a constant expression"
//@ diag: 30:14 note "declared here"
//@ diag: 52:24 error "invalid operands to binary expression ('It' and 'It')"
//@ diag: 53:34 error "invalid operands to binary expression ('const It' and 'const It')"
//@ diag: 55:31 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 55:33 note "read of non-constexpr variable 'a' is not allowed in a constant expression"
//@ diag: 54:15 note "declared here"
//@ diag: 61:37 error "variable 'c' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 58:25 note "'c' declared here"
//@ diag: 59:10 note "lambda expression begins here"
//@ diag: 59:11 note "capture 'c' by value"
//@ diag: 59:11 note "capture 'c' by reference"
//@ diag: 59:11 note "default capture by value"
//@ diag: 59:11 note "default capture by reference"
// The capture error is the only error at 61:37: the hidden `__range` is
// marked invalid rather than diagnosed again for its failed constant
// initialization (one error for one mistake).
struct NB {
  int e[2];
  const int *begin() const { return e; }
  constexpr const int *end() const { return e + 2; }
};
struct It {
  int v;
  constexpr int operator*() const { return v; }
  constexpr It operator+(int n) const { return {v + n}; }
  constexpr bool operator!=(It o) const { return v != o.v; }
};
struct NoMinus {
  constexpr It begin() const { return {0}; }
  constexpr It end() const { return {3}; }
};
template <class T, int N> struct Arr {
  T e[N];
  constexpr const T *begin() const { return e; }
  constexpr const T *end() const { return e + N; }
};
void f(int);
void g() {
  NB nb{{1, 2}};
  template for (auto x : nb) f(x);
  template for (auto x : NoMinus{}) f(x);
  template for (constexpr auto x : NoMinus{}) f(x);
  Arr<int, 2> a{{1, 2}};
  template for (constexpr int x : a) f(x);
}
int h() {
  constexpr Arr<int, 2> c{{1, 2}};
  return [] {
    int s = 0;
    template for (constexpr int x : c) s += x;
    return s;
  }();
}
