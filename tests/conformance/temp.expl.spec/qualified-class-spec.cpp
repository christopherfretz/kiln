// [temp.expl.spec]/2-3, [dcl.meaning.general]/2: a class template's
// explicit specialization may be declared with a qualified name in any
// enclosing namespace of the template; a partial specialization likewise.
// The specialized class is complete and usable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:1 error "extraneous 'template<>' in declaration of struct 'P'"
//@ diag: 14:23 error "specialization of member 'N::P' does not specialize an instantiated member"
//@ diag: 12:51 note "attempt to specialize declaration here"
//@ diag: 15:37 error "class template specialization of 'Z' not in a namespace enclosing 'N'"
//@ diag: 12:41 note "explicitly specialized declaration is here"
namespace N { template <class T> struct Z; struct P; }
template <> struct N::Z<int> {};
template <> struct N::P {};
namespace M { template <> struct N::Z<long> {}; }
template <class T> struct Q { template <class U> struct R; };
template <> template <> struct Q<int>::R<int> {};
namespace N { namespace I { template <class T> struct W; } }
template <> struct N::I::W<int> { int x; };
N::Z<int> z;
N::I::W<int> w;
int g() { return w.x; }
struct S { template <class T> struct V; };
template <> struct S::V<int> {};
template <class T> struct N::Z<T*> {};
