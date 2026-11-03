// [dcl.attr.deprecated]/4, [namespace.def]/1: a namespace definition's
// attributes apply to that definition; a use of a deprecated namespace (a
// nested-name-specifier, at its `::`; a using-directive; a namespace alias
// definition) warns while that definition is the latest one (a reopening
// does not inherit it, as in clang); not on an unnamed namespace (WP-303).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:13 error "an attribute list cannot appear here"
//@ diag: 36:13 warning "'deprecated' attribute on anonymous namespace ignored"
//@ diag: 41:10 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 42:12 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 43:17 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 44:16 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 48:10 warning "'I' is deprecated"
//@ diag: 37:20 note "'I' has been explicitly marked deprecated here"
//@ diag: 49:10 warning "'D' is deprecated"
//@ diag: 38:13 note "'D' has been explicitly marked deprecated here"
//@ diag: 50:25 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 52:13 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 52:25 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 53:38 warning "'P' is deprecated"
//@ diag: 33:13 note "'P' has been explicitly marked deprecated here"
//@ requires: clang>=22
// clang 22 spelling of namespace-deprecated.cpp (WP-394).
namespace [[deprecated("old")]] N { int x; namespace M { int y; } }
namespace [[deprecated]] P { int z; template <class T> struct S {}; int fn(int); }
namespace N { int w; }
namespace Q [[deprecated]] { int q; }
namespace [[deprecated]] { int anon; }
inline namespace [[deprecated]] I { int ii; }
namespace [[deprecated]] D { namespace E { int ee; } int in_d = P::z; }
int a = N::x;
int b = N::M::y;
int c = P::z;
int d = ::P::z;
using namespace P;
namespace PA = P;
int e = PA::z;
int f = Q::q;
int h = anon + ii;
int k = I::ii;
int m = D::E::ee;
namespace D { int dd = P::z; }
int n = D::dd;
struct R : P::S<int> { P::S<long> s; };
template <class T> void g() { (void)P::fn(0); }
