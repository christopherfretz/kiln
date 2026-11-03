// [expr.cast]/4, [expr.static.cast]/4, [dcl.init.ref]/5.4: a C-style or
// functional cast to a reference type is a static_cast before it is a
// reinterpret_cast, so `(const int&)c` for a `char c` binds the reference to
// a temporary `int` holding c's value (`const int& t(c);`), as does a cast
// to an unrelated class with a converting constructor or from a class with a
// conversion function. Only when no such initialization exists (a reference
// to volatile) is the glvalue reinterpreted. Verified against clang 19 and 22.
//@ expect: run
struct S { constexpr S(int v) : v(v) {} int v; };
struct C { constexpr operator int() const { return 7; } };
using CIR = const int&;
using IRR = int&&;
using CSR = const S&;
constexpr int via_c_style(char c) { return (const int&)c; }
constexpr int via_functional(char c) { return CIR(c); }
constexpr int via_rvalue(char c) { return IRR(c) + (int&&)c; }
constexpr int via_class(char c) { return ((const S&)c).v + CSR(c).v; }
constexpr int via_conversion(C k) { return (const int&)k; }
constexpr int via_prvalue() { return CIR(5); }
static_assert(via_c_style(3) == 3);
static_assert(via_functional(4) == 4);
static_assert(via_rvalue(2) == 4);
static_assert(via_class(6) == 12);
static_assert(via_conversion(C{}) == 7);
static_assert(via_prvalue() == 5);
int main() {
  char c = 9;
  volatile char vc = 11;
  long l = 1;
  if ((const int&)c != 9 || CIR(vc) != 11 || (const long long&)c != 9) return 1;
  // No initialization binds a reference to volatile: reinterpreted.
  if (&(volatile const long&)l != &l) return 2;
  return via_c_style(c) == 9 && via_class(c) == 18 ? 0 : 3;
}
