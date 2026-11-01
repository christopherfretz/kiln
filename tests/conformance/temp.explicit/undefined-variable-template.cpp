// [temp.explicit]/5: an explicit instantiation definition of a variable template
// specialization still undefined at the end of the TU is an error, reported at its
// first reference if odr-used before (clang's point of instantiation); a static
// data member is reported at the use and again at the explicit instantiation
// (clang PerformPendingInstantiations). A later definition is not an error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:14 error "explicit instantiation of undefined variable template 'v<int>'"
//@ diag: 23:30 note "explicit instantiation refers here"
//@ diag: 34:30 error "explicit instantiation of undefined static data member 'm' of class template 'S<int>'"
//@ diag: 32:41 note "explicit instantiation refers here"
//@ diag: 35:22 error "explicit instantiation of undefined static data member 'm' of class template 'S<int>'"
//@ diag: 32:41 note "explicit instantiation refers here"
//@ diag: 36:22 error "explicit instantiation of undefined static data member 'n' of class template 'S<int>'"
//@ diag: 32:55 note "explicit instantiation refers here"
//@ diag: 27:29 error "explicit instantiation of undefined variable template 'u<long>'"
//@ diag: 26:30 note "explicit instantiation refers here"
//@ diag: 31:17 error "explicit instantiation of undefined variable template 'N::q<char>'"
//@ diag: 30:44 note "explicit instantiation refers here"
//@ diag: 40:14 error "explicit instantiation of undefined variable template 'unev<int>'"
//@ diag: 38:30 note "explicit instantiation refers here"
// Undefined patterns, and a use of each:
template<class T> extern int v;
template int v<int>;
template<class T> int w;
template<class T> extern int u;
int use_u() { return sizeof(u<long>) + u<long>; }
template<class T> extern int late;
template int late<int>;
namespace N { template<class T> extern int q; }
template int N::q<char>;
template<class T> struct S { static int m; static int n; };
int use_w() { return w<char>; }
int use_m() { return S<int>::m; }
template int S<int>::m;
template int S<int>::n;
template int u<long>;
template<class T> extern int unev;
int use_unev() { return sizeof(unev<int>); }
template int unev<int>;
template<class T> int late = 5;
template int w<char>;
