// [basic.lookup.qual]/1: lookup of a name before `::` considers only
// namespaces, types and templates whose specializations are types; a name
// that ordinary lookup finds as something else (a variable, function,
// enumerator, data or function member, variable template, concept, local
// variable, a function named by a using-declaration) is not a class,
// namespace, or enumeration. clang notes the declaration when lookup found
// exactly one that is not a function template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:10 error "'v' is not a class, namespace, or enumeration"
//@ diag: 33:5 note "'v' declared here"
//@ diag: 42:10 error "'fn' is not a class, namespace, or enumeration"
//@ diag: 34:6 note "'fn' declared here"
//@ diag: 43:10 error "'e1' is not a class, namespace, or enumeration"
//@ diag: 35:10 note "'e1' declared here"
//@ diag: 44:13 error "'w' is not a class, namespace, or enumeration"
//@ diag: 36:19 note "'w' declared here"
//@ diag: 45:13 error "'m' is not a class, namespace, or enumeration"
//@ diag: 37:16 note "'m' declared here"
//@ diag: 46:13 error "'mf' is not a class, namespace, or enumeration"
//@ diag: 37:24 note "'mf' declared here"
//@ diag: 47:10 error "'ft' is not a class, namespace, or enumeration"
//@ diag: 48:10 error "'vt' is not a class, namespace, or enumeration"
//@ diag: 39:24 note "'vt' declared here"
//@ diag: 49:10 error "'Co' is not a class, namespace, or enumeration"
//@ diag: 40:28 note "'Co' declared here"
//@ diag: 50:22 error "'q' is not a class, namespace, or enumeration"
//@ diag: 36:27 note "'q' declared here"
//@ diag: 51:5 error "'v' is not a class, namespace, or enumeration"
//@ diag: 33:5 note "'v' declared here"
//@ diag: 52:31 error "'loc' is not a class, namespace, or enumeration"
//@ diag: 52:16 note "'loc' declared here"
int v;
void fn();
enum E { e1 };
namespace N { int w; void q(); }
struct S { int m; void mf(); };
template <class T> void ft();
template <class T> int vt = 0;
template <class T> concept Co = true;
int a1 = v::x;
int a2 = fn::x;
int a3 = e1::x;
int a4 = N::w::x;
int a5 = S::m::x;
int a6 = S::mf::x;
int a7 = ft::x;
int a8 = vt::x;
int a9 = Co::x;
using N::q; int b1 = q::x;
int v::*p1;
void g() { int loc = 0; (void)loc::x; }
