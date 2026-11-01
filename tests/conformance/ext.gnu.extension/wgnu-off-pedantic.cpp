// -pedantic -Wno-gnu: the -Wgnu umbrella switches off every GNU extension
// warning that -pedantic enabled (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ args: -pedantic -Wno-gnu
void g(int n, int i) {
  int a[n]; (void)a;
  switch (i) { case 1 ... 3: break; }
}
template <class C, C...> int operator""_s();
#line 0
