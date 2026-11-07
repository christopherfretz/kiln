// [dcl.constexpr]/3 (C++20): the non-literal-variable rule as clang 19's
// CheckConstexprFunctionStmt applies it looks into an if-statement's
// branches only, so its init-statement and condition variable may be of
// non-literal type (googletest's EXPECT_* declares an AssertionResult
// there), also in a member of a class template; a loop's condition
// variable is checked. WP-726.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:38 error "variable of non-literal type 'const NL' cannot be defined in a constexpr function before C++23"
//@ diag: 12:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
struct NL { NL(bool b) : b(b) {} ~NL() {} bool b; explicit operator bool() const { return b; } };
constexpr bool f(int x) {
  if (x == 1) return true;
  if (const NL r = (x > 2)) ; else return false;
  if (NL i(true); i) ; else return false;
  switch (0) case 0: default: if (const NL q = true) ; else return false;
  return true;
}
template <int N> struct T {
  constexpr bool g(int x) const { if (const NL r = (x > N)) ; else return false; return true; }
};
constexpr bool h() { while (const NL w = false) {} return true; }
static_assert(f(1));
int main() { return f(3) && T<1>{}.g(2) ? 0 : 1; }
