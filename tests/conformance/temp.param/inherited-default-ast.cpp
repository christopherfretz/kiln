// [temp.param]/12: a redeclaration's template parameter without a default
// inherits the previous declaration's default, owned by the parameter that
// wrote it (clang's `inherited from`), for type, non-type and template
// template parameters of class, function and variable templates. Defaults
// pass on only to later declarations (`S`'s first declaration inherits none).
//@ expect: ast
template<class> struct Q;
template<class T = int, int N = 3, template<class> class TT = Q> struct R;
template<class A, int M, template<class> class UU> struct R;
template<class B, int K, template<class> class VV> struct R {};
R<> r;
template<class T, class U = int> struct S;
template<class T = char, class U> struct S;
template<class X, class Y> struct S {};
S<> s;
template<class T = int> void f();
template<class U> void f() {}
struct P { template<class T, int N = 2> static const int v; };
template<class U, int M> const int P::v = M;
struct O { template<class T = long> void g(); };
template<class U> void O::g() {}
