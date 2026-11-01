// -Wgnu -Wno-gnu-case-range: a subgroup can be turned back off (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wgnu -Wno-gnu-case-range
//@ diag: 11:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 11:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 10:12 note "declared here"
//@ diag: 14:30 warning "string literal operator templates are a GNU extension"
//@ diag: 15:7 warning "#line directive with zero argument is a GNU extension"
void g(int n, int i) {
  int a[n]; (void)a;
  switch (i) { case 1 ... 3: break; }
}
template <class C, C...> int operator""_s();
#line 0
