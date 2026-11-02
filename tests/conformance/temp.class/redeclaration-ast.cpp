// [temp.class]/1: each redeclaration of a class template is its own
// ClassTemplateDecl in the redeclaration chain (clang: `prev`), with its own
// template parameter list (names as that declaration wrote them) and its own
// pattern CXXRecordDecl. Inherited default arguments are covered by
// redeclaration-diags.cpp (the dump normalizer has no `inherited from` yet).
//@ expect: ast
template<class T, int N> struct R;
template<class A, int M> struct R { A a[M]; R *p; };
template<class U, int K> struct R;
R<int, 2> r;
