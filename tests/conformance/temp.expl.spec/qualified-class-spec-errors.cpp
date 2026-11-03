// [temp.expl.spec]/2-3, [temp.spec.general]/3: a qualified explicit
// specialization must be in a namespace enclosing the template, name a
// declared class template of the nominated namespace, and not be
// defined twice.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:36 error "class template specialization of 'Z' must occur at global scope"
//@ diag: 22:27 note "explicitly specialized declaration is here"
//@ diag: 25:20 error "explicit specialization of non-template struct 'NT'"
//@ diag: 25:20 error "redefinition of 'NT'"
//@ diag: 24:8 note "previous definition is here"
//@ diag: 27:23 error "explicit specialization of non-template struct 'P2'"
//@ diag: 27:23 error "redefinition of 'P2'"
//@ diag: 26:22 note "previous definition is here"
//@ diag: 32:23 error "redefinition of 'Z3<int>'"
//@ diag: 31:23 note "previous definition is here"
//@ diag: 33:23 error "explicit specialization of non-template struct 'Q4'"
//@ diag: 33:23 error "no struct named 'Q4' in namespace 'N'"
//@ diag: 34:20 error "use of undeclared identifier 'NS5'"
//@ diag: 34:25 error "explicit specialization of non-template struct 'Z'"
namespace N2 { template <class T> struct Z; }
template <class T> struct Z;
namespace M { template <> struct ::Z<int> {}; }
struct NT {};
template <> struct NT<int> {};
namespace N { struct P2 {}; }
template <> struct N::P2<int> {};
namespace N { template <class T> struct Z3; }
namespace K { template <class T> struct Z3; }
template <> struct K::Z3<int>;
template <> struct N::Z3<int> { int a; };
template <> struct N::Z3<int> { int b; };
template <> struct N::Q4<int> {};
template <> struct NS5::Z<int> {};
