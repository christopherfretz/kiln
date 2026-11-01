// -Wgnu enables the GNU extension warnings: case ranges, VLAs, string literal
// operator templates, #line 0 (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wgnu
//@ diag: 13:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 13:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 12:12 note "declared here"
//@ diag: 14:23 warning "use of GNU case range extension"
//@ diag: 16:30 warning "string literal operator templates are a GNU extension"
//@ diag: 17:7 warning "#line directive with zero argument is a GNU extension"
void g(int n, int i) {
  int a[n]; (void)a;
  switch (i) { case 1 ... 3: break; }
}
template <class C, C...> int operator""_s();
#line 0
