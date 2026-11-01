// [dcl.attr.fallthrough] (WP-217): -Wimplicit-fallthrough is off by default
// and not in -Wall or -Wextra; nothing is diagnosed here.
//@ args: -Wall -Wextra
//@ strict-diags
int g(int);
void f(int x) {
  switch (x) {
  case 0:
    g(0);
  case 1:
    return;
    [[fallthrough]];
  default:
    g(1);
  }
}
