// [temp.expl.spec]/1: `template<>` before a qualified class name that
// is not a template-id: a class template needs its arguments, an
// undeclared name and a non-template class are rejected.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:1 error "extraneous 'template<>' in declaration of struct 'Z'"
//@ diag: 25:23 error "redefinition of 'Z' as different kind of symbol"
//@ diag: 24:41 note "previous definition is here"
//@ diag: 26:1 error "extraneous 'template<>' in declaration of struct 'U'"
//@ diag: 26:23 error "no struct named 'U' in namespace 'N'"
//@ diag: 27:20 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 27:1 error "extraneous 'template<>' in declaration of struct 'P'"
//@ diag: 27:23 error "specialization of member 'N::P' does not specialize an instantiated member"
//@ diag: 24:51 note "attempt to specialize declaration here"
//@ diag: 28:1 error "extraneous 'template<>' in declaration of struct 'D'"
//@ diag: 28:23 error "specialization of member 'N::D' does not specialize an instantiated member"
//@ diag: 24:61 note "attempt to specialize declaration here"
//@ diag: 30:20 error "explicit specialization of non-template struct 'NT'"
//@ diag: 32:23 error "explicit specialization of non-template struct 'P'"
//@ diag: 32:20 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 33:1 error "extraneous 'template<>' in declaration of class 'P'"
//@ diag: 33:22 error "specialization of member 'N::P' does not specialize an instantiated member"
//@ diag: 32:23 note "attempt to specialize declaration here"
namespace N { template <class T> struct Z; struct P; struct D {}; }
template <> struct N::Z {};
template <> struct N::U {};
template <> struct N::P;
template <> struct N::D {};
struct NT;
template <> struct NT<int> {};
NT *pnt;
template <> struct N::P<int>;
template <> class N::P final {};
