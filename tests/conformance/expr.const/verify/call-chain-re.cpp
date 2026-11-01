// [expr.const]/5.2, [dcl.constexpr]/3 (WP-143): calling a non-constexpr
// function in a constant evaluation is not a core constant expression;
// the call chain notes name the arguments, matched here with -re.
//@ verify
int runtime(int x) { return x; } // expected-note {{declared here}}
constexpr int f(int n) {
  return n < 3 ? f(n + 1) : runtime(n); // expected-note {{non-constexpr function 'runtime' cannot be used in a constant expression}}
  // expected-note-re@-1 {{in call to 'f({{[0-9]+}})'}}
}
static_assert(f(2) == 3); // expected-error-re {{static assertion expression is not an integral constant expression{{$}}}}
// expected-note-re@-1 {{in call to 'f({{[0-9]}})'}}
