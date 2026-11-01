// [expr.const]/5.8, [expr.mul]/4 (WP-143): division by zero in a constant
// evaluation makes the initializer non-constant; the reason is a note at
// the division, and the call chain a note at the call. Checked with -verify
// under kiln and clang.
//@ verify
constexpr int div(int a, int b) {
  return a / b; // expected-note {{division by zero}}
}
constexpr int ok = div(6, 3);
static_assert(ok == 2);
constexpr int bad = div(1, 0); // expected-error {{constexpr variable 'bad' must be initialized by a constant expression}} \
                               // expected-note {{in call to 'div(1, 0)'}}
