// [dcl.attr.deprecated]/4, [namespace.def]/1: a namespace definition's
// attributes apply to that definition; a use of a deprecated namespace (a
// nested-name-specifier, at its `::`; a using-directive; a namespace alias
// definition) warns while that definition is the latest one (a reopening
// does not inherit it, as in clang); not on an unnamed namespace (WP-303).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 37:13 warning "'deprecated' attribute on anonymous namespace ignored"
//@ diag: 42:10 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 43:12 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 44:17 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 45:16 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 47:10 warning "'Q' is deprecated"
//@ diag: 36:15 note "'Q' has been explicitly marked deprecated here"
//@ diag: 49:10 warning "'I' is deprecated"
//@ diag: 38:20 note "'I' has been explicitly marked deprecated here"
//@ diag: 50:10 warning "'D' is deprecated"
//@ diag: 39:13 note "'D' has been explicitly marked deprecated here"
//@ diag: 51:25 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 53:13 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 53:25 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ diag: 54:38 warning "'P' is deprecated"
//@ diag: 34:13 note "'P' has been explicitly marked deprecated here"
//@ requires: clang<22
// clang 22 expected compile-ok, but the compiler failed (exit code 1); WP-355 residual
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
