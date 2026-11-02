// [temp.explicit]/9, [except.spec]/2: an explicit instantiation
// declaration/definition need not repeat the template's exception
// specification. libstdc++ bits/locale_classes.tcc / codecvt.h:813
// `extern template bool has_facet<codecvt<...>>(const locale&);` where
// has_facet is declared `throw()`; <ios>, <iterator>, <streambuf>, 28 headers
// (docs/status/libstdcxx.md, WP-188). The instantiation takes the template's
// non-dependent specification (WP-294).
// Verified against clang 19.
//@ expect: compile-ok
struct locale {};
template<class F> bool has_facet(const locale &) throw();
template<class F> bool has_facet(const locale &) throw() { return true; }
struct C {};
extern template bool has_facet<C>(const locale &);
template bool has_facet<C>(const locale &);
template<class F> int c(F) noexcept { return 0; }
template int c<int>(int);
extern template int c<char>(char);
