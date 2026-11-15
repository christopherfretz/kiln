// C++11-C++20 [temp.explicit]/5 (WP-1448): an explicit instantiation whose
// name is unqualified shall appear in the namespace where its template is
// declared or, if that namespace is inline, in any namespace of its
// enclosing namespace set (clang err_explicit_instantiation_unqualified_wrong_namespace,
// err_explicit_instantiation_must_be_global). C++23 dropped the rule (P1787):
// scope-unqualified-cxx26.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 22:53 error "explicit instantiation of 'N::E' must occur in namespace 'N'"
//@ diag: 17:28 note "explicit instantiation refers here"
//@ diag: 23:43 error "explicit instantiation of 'N::E' must occur in namespace 'N'"
//@ diag: 17:28 note "explicit instantiation refers here"
//@ diag: 24:31 error "explicit instantiation of 'Glob' must occur at global scope"
//@ diag: 21:26 note "explicit instantiation refers here"
namespace N {
  template<class T> struct E {};
  inline namespace In { template<class T> struct G {}; template<class T> void h(T) {} }
  namespace Q { template<class T> struct H {}; }
}
template<class T> struct Glob {};
namespace N { inline namespace In { template struct E<int>; } }
namespace M { using N::E; template struct E<long>; }
namespace M { template struct Glob<int>; }

// Valid: the template's namespace, or a namespace of an inline
// namespace's enclosing namespace set.
namespace N { template struct E<char>; template struct G<int>; template void h<int>(int); }
namespace N { inline namespace In { template struct G<long>; } }
namespace N { namespace Q { template struct H<char>; } }
template struct Glob<char>;
