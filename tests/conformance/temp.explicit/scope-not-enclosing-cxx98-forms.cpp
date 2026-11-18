// C++98 (WP-1692): the explicit instantiation scope rule of C++11
// [temp.explicit]/3 (DR 275, which relaxed C++03 [temp.explicit]/5's "the
// namespace of the template" to an enclosing namespace; for an unqualified
// name the template's own namespace) for every kind of entity: class and
// function templates, member functions, member classes, static data
// members, member function templates, an `extern template` declaration
// and a global template; and, in every mode, an explicit instantiation in
// a class (no member-declaration is one: clang's "expected '<' after
// 'template'"; kiln accepted it before). GCC 16.1 rejects each line in
// C++98 as in C++11; clang 22 gives clang's C++11 texts there as its
// off-by-default -Wc++11-compat warnings, which kiln makes errors (the
// C++11 rule, as GCC). Variable templates: scope-qualified-not-enclosing.cpp.
//@ expect: compile-fail
//@ std: c++98
//@ oracle: gcc
//@ strict-diags
//@ xfail(clang): clang 22 only warns under -Wc++11-compat in C++98
//@ diag: 53:22 error "explicit instantiation of 'E' not in a namespace enclosing 'N'"
//@ diag: 44:28 note "explicit instantiation refers here"
//@ diag: 54:20 error "explicit instantiation of 'f' not in a namespace enclosing 'N'"
//@ diag: 45:26 note "explicit instantiation refers here"
//@ diag: 55:22 error "explicit instantiation of 'C' not in a namespace enclosing 'N'"
//@ diag: 46:28 note "explicit instantiation refers here"
//@ diag: 56:29 error "explicit instantiation of 'm' not in a namespace enclosing 'N'"
//@ diag: 46:37 note "explicit instantiation refers here"
//@ diag: 57:32 error "explicit instantiation of 'I' not in a namespace enclosing 'N'"
//@ diag: 46:51 note "explicit instantiation refers here"
//@ diag: 58:27 error "explicit instantiation of 's' not in a namespace enclosing 'N'"
//@ diag: 46:68 note "explicit instantiation refers here"
//@ diag: 59:30 error "explicit instantiation of 'mt' not in a namespace enclosing 'N'"
//@ diag: 46:94 note "explicit instantiation refers here"
//@ diag: 60:29 error "explicit instantiation of 'E' not in a namespace enclosing 'N'"
//@ diag: 44:28 note "explicit instantiation refers here"
//@ diag: 61:21 error "explicit instantiation of 'Glob' must occur at global scope"
//@ diag: 50:26 note "explicit instantiation refers here"
//@ diag: 64:22 error "explicit instantiation of 'E' not in a namespace enclosing 'N'"
//@ diag: 44:28 note "explicit instantiation refers here"
//@ diag: 66:43 error "explicit instantiation of 'N::E' must occur in namespace 'N'"
//@ diag: 44:28 note "explicit instantiation refers here"
//@ diag: 67:31 error "explicit instantiation of 'Glob' must occur at global scope"
//@ diag: 50:26 note "explicit instantiation refers here"
//@ diag: 68:21 error "expected '<' after 'template'"
namespace N {
  template<class T> struct E {};
  template<class T> void f(T) {}
  template<class T> struct C { void m() {} struct I {}; static int s; template<class U> void mt(U) {} };
  template<class T> int C<T>::s = 0;
  namespace Inner {}
}
template<class T> struct Glob {};

namespace M { // contains none of the entities below
  template struct N::E<long>;
  template void N::f<long>(long);
  template struct N::C<long>;
  template void N::C<char>::m();
  template struct N::C<short>::I;
  template int N::C<int>::s;
  template void N::C<float>::mt<int>(int);
  extern template struct N::E<signed char>;
  template struct ::Glob<long>;
}
namespace N { namespace Inner {
  template struct N::E<int>; // Inner is inside N, but does not contain E
} }
namespace M { using N::E; template struct E<unsigned long>; }
namespace M { template struct Glob<int>; }
struct K { template struct N::E<double>; };

// Valid: the global scope, the template's namespace.
template struct N::E<char>;
template void N::f<char>(char);
template void N::C<double>::m();
template struct Glob<char>;
template struct ::Glob<short>;
namespace N {
  template struct E<short>;
  template struct N::E<unsigned>;
  template int C<long long>::s;
  extern template struct E<unsigned char>;
}
