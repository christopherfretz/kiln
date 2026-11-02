// [dcl.attr.deprecated]/4, [temp.names] (WP-297): a template-id naming a
// specialization of a deprecated class template warns at the template-name
// with the arguments as written plus defaults; an elaborated-type-specifier
// warns; the qualifier of an out-of-line member definition of a deprecated
// explicit specialization does not (the member is in a deprecated context);
// class template argument deduction does not warn.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 28:8 warning "'S' is deprecated"
//@ diag: 27:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 31:1 warning "'D<char>' is deprecated"
//@ diag: 30:43 note "'D<char>' has been explicitly marked deprecated here"
//@ diag: 32:1 warning "'D<N::X>' is deprecated"
//@ diag: 30:43 note "'D<N::X>' has been explicitly marked deprecated here"
//@ diag: 33:1 warning "'D<char, long>' is deprecated"
//@ diag: 30:43 note "'D<char, long>' has been explicitly marked deprecated here"
//@ diag: 34:16 warning "'S' is deprecated"
//@ diag: 27:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 35:10 warning "'Q<char>' is deprecated"
//@ diag: 24:21 note "'Q<char>' has been explicitly marked deprecated here"
//@ diag: 37:7 warning "'P<int>' is deprecated"
//@ diag: 36:28 note "'P<int>' has been explicitly marked deprecated here"
template<class T> struct Q { void h(); };
template<> struct [[deprecated]] Q<char> { void h(); static int v; };
void Q<char>::h() {}
int Q<char>::v = 0;
struct [[deprecated]] S {};
struct S s2;
namespace N { struct X {}; }
template<class T, class U = int> struct [[deprecated]] D {};
D<char> d1;
D<N::X> d2;
D<char, long> d3;
int k = sizeof(S);
auto l = Q<char>::v;
template<class T> struct [[deprecated]] P {};
P p = P<int>{};
