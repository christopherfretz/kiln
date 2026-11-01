// [namespace.udecl]/10, [basic.scope.scope]/4, [namespace.memdef]/3: a
// hidden friend `swap(stop_token&, stop_token&)` and a using-declaration of a
// different `swap` overload in the same namespace do not conflict. libstdc++
// stop_token:95 vs bits/exception_ptr.h:242 (`using __exception_ptr::swap;`);
// <stop_token>, <thread>, 7 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): redefinition of 'swap' as different kind of symbol
namespace std {
namespace ep {
  struct exception_ptr { int p = 0; };
  inline void swap(exception_ptr &, exception_ptr &) noexcept {}
}
using ep::swap;
struct stop_token {
  int s = 0;
  friend void swap(stop_token &a, stop_token &b) noexcept { int t = a.s; a.s = b.s; b.s = t; }
};
}
void f(std::stop_token &a, std::stop_token &b, std::ep::exception_ptr &c) { swap(a, b); swap(c, c); }
