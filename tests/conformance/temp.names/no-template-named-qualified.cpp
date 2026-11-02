// [temp.names]/3, [basic.lookup.qual]: at namespace or class scope a
// qualified name that names no member, followed by `<`, in a
// decl-specifier is diagnosed "no template named 'X' in ..." and its
// template argument list is skipped; the declarators are not diagnosed
// again (`std::codecvt_utf8<wchar_t>` removed in C++26).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:4 error "no template named 'foo' in namespace 'N'"
//@ diag: 17:4 error "no template named 'bar' in 'C'"
//@ diag: 18:12 error "no template named 'zap' in 'N::Q<int>'"
//@ diag: 19:4 error "no template named 'foo' in namespace 'N'"
//@ diag: 21:6 error "no template named 'foo' in namespace 'N'"
//@ diag: 22:6 error "no template named 'bar' in 'C'"
namespace N { struct S {}; template <class T> struct Q {}; }
struct C {};
N::foo<int> a;
C::bar<int> b;
N::Q<int>::zap<int> z;
N::foo<int> *pp, qq;
struct X {
  N::foo<int> m;
  C::bar<int, N::S> *p;
};
N::Q<int> ok;
