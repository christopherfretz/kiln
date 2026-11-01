// [temp.explicit]/9, [except.spec]/2: an explicit instantiation
// declaration/definition need not repeat the template's exception
// specification. libstdc++ bits/locale_classes.tcc / codecvt.h:813
// `extern template bool has_facet<codecvt<...>>(const locale&);` where
// has_facet is declared `throw()`; <ios>, <iterator>, <streambuf>, 31 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): explicit instantiation without the template's throw() does not match
struct locale {};
template<class F> bool has_facet(const locale &) throw();
template<class F> bool has_facet(const locale &) throw() { return true; }
struct C {};
extern template bool has_facet<C>(const locale &);
template bool has_facet<C>(const locale &);
