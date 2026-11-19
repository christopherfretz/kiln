// C++98 [temp.explicit]/5 (C++03 14.7.2/5): an explicit instantiation
// whose name is unqualified appears in the namespace of the template;
// DR 275 (C++11 [temp.explicit]/3) kept that for an unqualified name, so
// a name found at the global scope through a using-declaration or a
// using-directive is ill-formed in every mode before C++23 (WP-1693).
// clang 19/22 give only their off-by-default -Wc++11-compat warnings in
// C++98, and GCC 16.1 accepts these in C++98 (a pedantic error from
// C++11): kiln follows the text (docs/status/deviations.md). Reduced from
// GCC's g++.dg/cpp0x/explicit-inst1.C.
//@ expect: compile-fail
//@ std: c++98
//@ xfail(clang): clang 19 and 22 accept in C++98 (warn_explicit_instantiation_unqualified_wrong_namespace_0x, off by default)
//@ diag: 23:16 error "explicit instantiation of 'N::bar' must occur in namespace 'N'"
//@ diag: 19:28 note "explicit instantiation refers here"
//@ diag: 26:16 error "explicit instantiation of 'N::foo' must occur in namespace 'N'"
//@ diag: 18:28 note "explicit instantiation refers here"
namespace N {
  template <class T> class foo {};
  template <class T> class bar {};
}

using N::bar;
template class bar<int>;

using namespace N;
template class foo<int>;
