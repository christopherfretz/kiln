// [expr.prim.id.qual]/3 (WP-1448): an explicit instantiation whose name is
// qualified must inhabit a scope that contains the entity instantiated, that
// is a namespace enclosing the namespace of the template (or of the class
// template of a member). Class, function and variable templates, member
// functions, member classes, static data members and member function
// templates; clang err_explicit_instantiation_out_of_scope, and
// err_explicit_instantiation_must_be_global for a global template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:22 error "explicit instantiation of 'E' not in a namespace enclosing 'N'"
//@ diag: 33:28 note "explicit instantiation refers here"
//@ diag: 45:20 error "explicit instantiation of 'f' not in a namespace enclosing 'N'"
//@ diag: 34:26 note "explicit instantiation refers here"
//@ diag: 46:19 error "explicit instantiation of 'v<long>' not in a namespace enclosing 'N'"
//@ diag: 35:25 note "explicit instantiation refers here"
//@ diag: 47:22 error "explicit instantiation of 'C' not in a namespace enclosing 'N'"
//@ diag: 36:28 note "explicit instantiation refers here"
//@ diag: 48:29 error "explicit instantiation of 'm' not in a namespace enclosing 'N'"
//@ diag: 36:37 note "explicit instantiation refers here"
//@ diag: 49:32 error "explicit instantiation of 'I' not in a namespace enclosing 'N'"
//@ diag: 36:51 note "explicit instantiation refers here"
//@ diag: 50:27 error "explicit instantiation of 's' not in a namespace enclosing 'N'"
//@ diag: 36:68 note "explicit instantiation refers here"
//@ diag: 51:30 error "explicit instantiation of 'mt' not in a namespace enclosing 'N'"
//@ diag: 36:94 note "explicit instantiation refers here"
//@ diag: 52:29 error "explicit instantiation of 'E' not in a namespace enclosing 'N'"
//@ diag: 33:28 note "explicit instantiation refers here"
//@ diag: 53:21 error "explicit instantiation of 'Glob' must occur at global scope"
//@ diag: 41:26 note "explicit instantiation refers here"
//@ diag: 56:22 error "explicit instantiation of 'E' not in a namespace enclosing 'N'"
//@ diag: 33:28 note "explicit instantiation refers here"
namespace N {
  template<class T> struct E {};
  template<class T> void f(T) {}
  template<class T> int v = 0;
  template<class T> struct C { void m() {} struct I {}; static int s; template<class U> void mt(U) {} };
  template<class T> int C<T>::s = 0;
  inline namespace In { template<class T> struct G {}; }
  namespace Inner {}
}
template<class T> struct Glob {};

namespace M { // contains none of the entities below
  template struct N::E<long>;
  template void N::f<long>(long);
  template int N::v<long>;
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

// Valid: the global scope, the template's namespace, and a namespace that
// encloses an inline namespace's member.
template struct N::E<char>;
template void N::f<char>(char);
template int N::v<char>;
template void N::C<double>::m();
template struct Glob<char>;
template struct ::Glob<short>;
namespace N {
  template struct E<short>;
  template struct N::E<unsigned>;
  template struct G<int>;
  template struct In::G<char>;
  template int C<long long>::s;
  extern template struct E<unsigned char>;
}
namespace N { inline namespace In { template struct N::G<long>; } }
