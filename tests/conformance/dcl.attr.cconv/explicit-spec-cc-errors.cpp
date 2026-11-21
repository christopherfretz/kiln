// [temp.expl.spec] [clang ext] An explicit specialization of a function
// template, or of a member function of a class template, that names a
// calling convention other than the template's (or the member's) is
// matched with the template's convention adopted (clang
// `adjustCCAndNoReturn`) and then merged as a redeclaration (clang
// `MergeFunctionDecl`): `function declared 'X' here was previously
// declared ...`, noted at the specialization itself when it is first
// declared there, else at the previous declaration (the member's).
// Reduced from real code. WP-1843.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:18 error "function declared 'preserve_most' here was previously declared without calling convention"
//@ diag: 37:18 note "previous declaration is here"
//@ diag: 39:21 error "function declared 'preserve_all' here was previously declared without calling convention"
//@ diag: 39:21 note "previous declaration is here"
//@ diag: 41:21 error "function declared 'preserve_most' here was previously declared 'preserve_all'"
//@ diag: 41:21 note "previous declaration is here"
//@ diag: 43:21 error "function declared 'preserve_most' here was previously declared 'preserve_all'"
//@ diag: 42:18 note "previous declaration is here"
//@ diag: 45:45 error "function declared 'swiftcall' here was previously declared without calling convention"
//@ diag: 45:45 note "previous declaration is here"
//@ diag: 46:18 error "function declared 'swiftcall' here was previously declared without calling convention"
//@ diag: 46:18 note "previous declaration is here"
//@ diag: 48:24 error "function declared 'preserve_most' here was previously declared without calling convention"
//@ diag: 48:24 note "previous declaration is here"
//@ diag: 50:29 error "function declared 'preserve_most' here was previously declared without calling convention"
//@ diag: 49:36 note "previous declaration is here"
//@ diag: 51:30 error "function declared 'preserve_all' here was previously declared 'preserve_most'"
//@ diag: 49:49 note "previous declaration is here"
//@ diag: 52:41 error "function declared 'preserve_most' here was previously declared without calling convention"
//@ diag: 52:41 note "previous declaration is here"
//@ diag: 54:24 error "function declared 'preserve_all' here was previously declared 'preserve_most'"
//@ diag: 54:24 note "previous declaration is here"
#define PM __attribute__((preserve_most))
#define PA __attribute__((preserve_all))
template <int N> void f() {}
template <> void f<2>() PM {}
template <int N> void h() {}
template <> PA void h<1>() {}
template <int N> PA void k() {}
template <> PM void k<1>() {}
template <> void k<2>();
template <> PM void k<2>() {}
template <class T> void s(T) {}
template <> __attribute__((swiftcall)) void s<int>(int) {}
template <> void s(long) __attribute__((swiftcall));
struct M { template <int N> void m(); };
template <> PM void M::m<1>() {}
template <class T> struct A { void f(); PM void g(); template <class U> void h(U); };
template <> PM void A<int>::f() {}
template <> PA void A<long>::g() {}
template <> template <> PM void A<int>::h(int) {}
struct B { template <class U> PM static void h(U); };
template <> PA void B::h(long) {}
