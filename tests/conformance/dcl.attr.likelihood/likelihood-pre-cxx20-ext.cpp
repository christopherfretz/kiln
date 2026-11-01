//@ expect: compile-ok
//@ std: c++14
//@ args: -pedantic
//@ strict-diags
//@ diag: 14:19 warning "use of the 'fallthrough' attribute is a C++17 extension"
//@ diag: 15:13 warning "use of the 'likely' attribute is a C++20 extension"
//@ diag: 17:5 warning "use of the 'unlikely' attribute is a C++20 extension"
// [dcl.attr.likelihood], [dcl.attr.fallthrough] (WP-182): before C++17
// (C++20) the attributes are accepted as extensions; -pedantic warns
// (-Wc++17-attribute-extensions, -Wc++20-attribute-extensions).
int g(int);
int f(int x) {
  switch (x) {
  case 1: g(3); [[fallthrough]];
  case 2: [[likely]] g(4); break;
  }
  [[unlikely]] return 0;
}
