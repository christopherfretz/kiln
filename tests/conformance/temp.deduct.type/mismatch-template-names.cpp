// [temp.deduct.type]/8, [temp.deduct.call]/4: the "could not match 'X' against
// 'Y'" note names the argument's template qualified by the outermost written
// qualifier (`s::pr` through a qualified placeholder, `t7::pr` through
// `t7::Q`) and bare through an unqualified placeholder or typedef (`pr`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:12 error "no matching function for call to 'f'"
//@ diag: 21:25 note "candidate template ignored: could not match 'mp' against 's::pr'"
//@ diag: 22:51 error "no matching function for call to 'f'"
//@ diag: 21:25 note "candidate template ignored: could not match 'mp' against 's::pr'"
//@ diag: 22:76 error "no matching function for call to 'f'"
//@ diag: 21:25 note "candidate template ignored: could not match 'mp' against 'pr'"
//@ diag: 28:13 error "no matching function for call to 'f7'"
//@ diag: 27:25 note "candidate template ignored: could not match 'mp7' against 'pr'"
//@ diag: 28:22 error "no matching function for call to 'f7'"
//@ diag: 27:25 note "candidate template ignored: could not match 'mp7' against 't7::pr'"
//@ diag: 28:54 error "no matching function for call to 'f7'"
//@ diag: 27:25 note "candidate template ignored: could not match 'mp7' against 's7::pr'"
namespace s { template <class A, class B> struct pr { pr(A, B); }; }
template <class K> struct mp {};
template <class K> void f(mp<K>);
void g() { f(s::pr{1, 2}); s::pr<int,int> x(1,2); f(x); using namespace s; f(pr{1,2}); }
namespace s7 { template <class A, class B> struct pr {}; }
namespace t7 { using Q = s7::pr<int, int>; }
using P = s7::pr<int, long>;
template <class K> struct mp7 {};
template <class K> void f7(mp7<K>);
void g7() { f7(P{}); f7(t7::Q{}); s7::pr<int,int> x; f7(x); }
