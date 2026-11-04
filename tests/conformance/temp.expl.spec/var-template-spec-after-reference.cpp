// [temp.expl.spec]/7, [temp.point]/1: a variable template specialization
// named before its explicit specialization is implicitly instantiated at
// that reference (evaluated or not, odr-use or not, before or after the
// template's definition), so the explicit specialization that follows is
// an error, as clang reports it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:27 error "explicit specialization of 'v' after instantiation"
//@ diag: 17:18 note "implicit instantiation first required here"
//@ diag: 27:23 error "explicit specialization of 'u' after instantiation"
//@ diag: 25:18 note "implicit instantiation first required here"
//@ diag: 31:17 error "explicit specialization of 'nc' after instantiation"
//@ diag: 30:18 note "implicit instantiation first required here"
//@ diag: 35:27 error "explicit specialization of 'uv' after instantiation"
//@ diag: 34:20 note "implicit instantiation first required here"
template <class T> constexpr int v = 1;
int f() { return v<int>; }
template <> constexpr int v<int> = 2;

template <class T> extern const int e;
int g() { return e<int>; }
template <class T> const int e = sizeof(T);

template <class T> extern const int u;
int h() { return u<char>; }
template <class T> const int u = 3;
template <> const int u<char> = 4;

template <class T> int nc = 1;
int i() { return nc<int>; }
template <> int nc<int> = 2;

template <class T> constexpr int uv = 1;
using X = decltype(uv<long>);
template <> constexpr int uv<long> = 2;
