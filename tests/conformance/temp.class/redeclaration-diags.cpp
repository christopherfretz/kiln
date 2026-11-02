// [temp.class]/1, [temp.param]/12, [temp.arg.general]/4: each class template
// redeclaration is its own declaration. Default arguments merge across all of
// them, a later redeclaration may leave them out, and the notes point at the
// most recent redeclaration (clang: 'template is declared here').
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:1 error "too many template arguments for class template 'S'"
//@ diag: 16:35 note "template is declared here"
//@ diag: 20:1 error "too few template parameters in template redeclaration"
//@ diag: 16:1 note "previous template declaration is here"
//@ diag: 29:8 error "implicit instantiation of undefined template 'D<int>'"
//@ diag: 27:26 note "template is declared here"
template<class T, class U = int> struct S;
template<class T = char, class U> struct S;
template<class T, class U> struct S { T t; U u; };
template<class A, class B> struct S;
S<> s;
static_assert(sizeof(s) == 8);
S<int, int, int> bad;
template<int N> struct S;
template<class T> struct R;
template<class T> struct R { R *p; R<T> *q; };
template<class T> struct R;
R<int> r;
static_assert(sizeof(R<long>) == 2 * sizeof(void *));
template<class T> struct D;
template<class T> struct D;
D<int> *dp;
D<int> dd;
template<class T> struct Q { template<class U> struct I; template<class U> struct I; };
template<class T> template<class U> struct Q<T>::I { U u; };
Q<int>::I<int> qi;
static_assert(sizeof(qi) == sizeof(int));
