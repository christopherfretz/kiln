// [expr.const]/16: an immediate invocation that is not a constant
// expression is diagnosed even when overload resolution for the enclosing
// operator substituted a default template argument finished as a
// constant-expression of its own (before WP-286 kiln dropped the pending
// invocation and accepted the call). Matches clang 19.
// Pinned to C++23: in C++26 a caught or uncaught throw is evaluated (P3068).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:11 error "call to consteval function 'Str::Str' is not a constant expression"
//@ diag: 23:70 note "subexpression not valid in a constant expression"
//@ diag: 27:11 note "in call to 'Str(&\"abcd\"[0])'"
template <bool B, class T = int> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T> struct is_code { static constexpr bool value = false; };
struct Code {
  template <class E, typename enable_if<is_code<E>::value>::type = 0>
  Code(E) {}
};
bool operator==(const Code&, const Code&) { return false; }
struct Str {
  int n;
  consteval Str(const char* s) : n(0) { while (s[n]) ++n; if (n > 3) throw 0; }
};
bool operator==(const Str& a, int b) { return a.n == b; }
int main() {
  return (Str("abcd") == 4) ? 0 : 1;
}
