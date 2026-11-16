// [stmt.expand]/5.2: N is counted by `for (auto b = begin-expr; b != e;
// ++b)` and each element is `*(begin + decltype(begin - begin){i})`, so
// every one of these expressions must be valid for the iterator: one
// without `++`, without unary `*` or without `!=` is an error naming the
// missing operation, constexpr for-range-declaration or not (GCC 16.1
// rejects the same; found running GCC's C++ testsuite).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 42:33 error "cannot increment value of type 'NoInc'"
//@ diag: 43:24 error "cannot increment value of type 'NoInc'"
//@ diag: 44:33 error "indirection requires pointer operand ('const NoDeref' invalid)"
//@ diag: 45:33 error "invalid operands to binary expression ('const NoNe' and 'const NoNe')"
struct NoInc {
  int i;
  constexpr int operator*() const { return i; }
  constexpr NoInc operator+(long d) const { return {int(i + d)}; }
  constexpr long operator-(NoInc o) const { return i - o.i; }
  constexpr bool operator!=(NoInc o) const { return i != o.i; }
};
struct NoDeref {
  int i;
  constexpr NoDeref &operator++() { ++i; return *this; }
  constexpr NoDeref operator+(long d) const { return {int(i + d)}; }
  constexpr long operator-(NoDeref o) const { return i - o.i; }
  constexpr bool operator!=(NoDeref o) const { return i != o.i; }
};
struct NoNe {
  int i;
  constexpr int operator*() const { return i; }
  constexpr NoNe &operator++() { ++i; return *this; }
  constexpr NoNe operator+(long d) const { return {int(i + d)}; }
  constexpr long operator-(NoNe o) const { return i - o.i; }
};
template <class I> struct R {
  constexpr I begin() const { return {0}; }
  constexpr I end() const { return {2}; }
};
void g(int);
void f() {
  template for (constexpr int x : R<NoInc>{}) { g(x); }
  template for (auto x : R<NoInc>{}) { g(x); }
  template for (constexpr int x : R<NoDeref>{}) { g(x); }
  template for (constexpr int x : R<NoNe>{}) { g(x); }
}
