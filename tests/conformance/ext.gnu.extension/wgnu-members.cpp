// Clang extensions, -Wgnu: every subgroup kiln implements, in clang 19's
// order (DiagnosticGroups.td GNU): gnu-case-range, gnu-designator,
// vla-extension (> vla-cxx-extension), gnu-folding-constant,
// gnu-imaginary-constant, gnu-include-next, gnu-line-marker,
// gnu-pointer-arith, gnu-string-literal-operator-template,
// gnu-zero-line-directive, gnu-zero-variadic-macro-arguments (WP-230).
// Probe: clang++-19 -fsyntax-only -std=c++26 -Wno-everything -Wgnu on this
// file gives exactly the diagnostics below. gnu-include-next needs a
// header, gnu-pointer-arith is an error in C++, gnu-zero-line-directive is
// in wgnu-umbrella.cpp. The GNU subgroups kiln lacks are listed in
// spec/diag.md section 3.1.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-everything -Wgnu
//@ diag: 29:19 warning "use of GNU array range extension"
//@ diag: 31:16 warning "expression is not an integral constant expression; folding it to a constant is a GNU extension"
//@ diag: 31:21 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 30:14 note "declared here"
//@ diag: 27:26 warning "token pasting of ',' and __VA_ARGS__ is a GNU extension"
//@ diag: 34:23 warning "imaginary constants are a GNU extension"
//@ diag: 35:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 35:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 32:12 note "declared here"
//@ diag: 36:23 warning "use of GNU case range extension"
//@ diag: 38:30 warning "string literal operator templates are a GNU extension"
//@ diag: 39:6 warning "this style of line directive is a GNU extension"
#define F(fmt, ...) f(fmt, ##__VA_ARGS__)
void f(const char *, ...);
int arr[3] = { [0 ... 1] = 4 };
const double d = 2.0;
enum En { en = (int)d };
void g(int n, int i) {
  F("a");
  _Complex double c = 2.0i; (void)c;
  int a[n]; (void)a;
  switch (i) { case 1 ... 3: break; }
}
template <class C, C...> int operator""_s();
# 40 "gnu.cpp"
