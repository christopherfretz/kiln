// [dcl.attr.fallthrough] (WP-203): `[[clang::fallthrough]]` is accepted
// in C++, and a scoped spelling (`clang::`, `gnu::`) is no C++17 extension
// before C++17 (clang `handleFallThroughAttr` warns for the unscoped one
// only).
//@ expect: compile-ok
//@ std: c++14
//@ args: -pedantic
//@ strict-diags
//@ diag: 20:7 warning "use of the 'fallthrough' attribute is a C++17 extension"
int f(int x) {
  switch (x) {
  case 0:
    x++;
    [[clang::fallthrough]];
  case 1:
    x++;
    [[gnu::fallthrough]];
  case 2:
    x++;
    [[fallthrough]];
  default:
    return x;
  }
}
