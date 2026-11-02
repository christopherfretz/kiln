// [expr.const]/16, [temp.deduct.general]/5: an immediate invocation stays
// pending until the end of its full-expression when overload resolution
// for an enclosing operator substitutes a default template argument (here
// a converting constructor template's `enable_if<...>::type = 0`, as in
// libc++ `error_code`, visible with `<ios>`) whose constant-expression is
// finished on its own; the consteval constructor is evaluated, not
// emitted as a call (libc++ `basic_format_string`; WP-286). Matches
// clang 19.
//@ expect: run
//@ exit: 7
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
  consteval Str(const char* s) : n(0) { while (s[n]) ++n; }
};
bool operator==(const Str& a, int b) { return a.n == b; }
template <class C> int in_template() { return Str("ab") == 2 ? 4 : 0; }
int main() {
  return ((Str("abc") == 3) ? 3 : 0) + in_template<char>();
}
