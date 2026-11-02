// [temp.arg.nontype]/1, [temp.names]: an enumeration-typed template
// argument names its enumerator, qualified (`S<n::E::b>`, unscoped
// `T<n::ub>`); a value with no enumerator prints bare (`S<7>`) or, for an
// `auto` parameter, as a cast to the canonical type (`A<(n::E)5>`); a
// character argument prints as a literal (`Ch<'x'>`) (WP-303).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:1 warning "'Ch<'x'>' is deprecated"
//@ diag: 28:28 note "'Ch<'x'>' has been explicitly marked deprecated here"
//@ diag: 36:1 warning "'S<n::E::b>' is deprecated"
//@ diag: 29:28 note "'S<n::E::b>' has been explicitly marked deprecated here"
//@ diag: 37:1 warning "'T<n::ub>' is deprecated"
//@ diag: 30:28 note "'T<n::ub>' has been explicitly marked deprecated here"
//@ diag: 38:1 warning "'W<n::C::M::m>' is deprecated"
//@ diag: 31:31 note "'W<n::C::M::m>' has been explicitly marked deprecated here"
//@ diag: 39:1 warning "'A<n::E::a>' is deprecated"
//@ diag: 32:28 note "'A<n::E::a>' has been explicitly marked deprecated here"
//@ diag: 40:1 warning "'A<(n::E)5>' is deprecated"
//@ diag: 32:28 note "'A<(n::E)5>' has been explicitly marked deprecated here"
//@ diag: 41:1 warning "'S<7>' is deprecated"
//@ diag: 29:28 note "'S<7>' has been explicitly marked deprecated here"
//@ diag: 42:12 error "call to deleted function 'f'"
//@ diag: 33:24 note "candidate function [with e = n::E::b] has been explicitly deleted"
//@ diag: 34:44 error "static assertion failed due to requirement '(n::E)0 == n::E::b'"
//@ diag: 43:12 note "in instantiation of template class 'Q<n::E::a>' requested here"
//@ diag: 34:46 note "expression evaluates to '0 == 1'"
namespace n { enum class E { a, b }; enum U { ua, ub }; struct C { enum class M { m }; }; using EE = E; }
template <char c> struct [[deprecated]] Ch {};
template <n::E e> struct [[deprecated]] S {};
template <n::U u> struct [[deprecated]] T {};
template <n::C::M m> struct [[deprecated]] W {};
template <auto v> struct [[deprecated]] A {};
template <n::E e> void f() = delete;
template <n::E e> struct Q { static_assert(e == n::E::b); };
Ch<'x'> c;
S<n::E::b> s;
T<n::ub> t;
W<n::C::M::m> w;
A<n::E::a> a;
A<(n::EE)5> a5;
S<(n::E)7> s7;
void g() { f<n::E::b>(); }
Q<n::E::a> q;
