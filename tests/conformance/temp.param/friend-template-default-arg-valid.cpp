// [temp.param]/12 allows a default template argument on a friend function
// template definition, and on member templates and namespace-scope
// templates; a friend template without defaults is fine, and a friend class
// template in a class template is not checked (as clang). Invalid pair:
// friend-template-default-arg-forms.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-ok
template <class T> struct R;
struct S {
  template <class T = int> friend int def(S, T = T()) { return 1; }
  template <class T> friend void decl(T);
  template <class T> friend struct R;
  template <class T = int> struct M {};
  template <class T = int> int mf() { return 2; }
};
template <class T> struct X {
  template <class U = int> friend struct Y;
  template <class U = T, int N = 2> friend int z(X, U = U()) { return N; }
};
template <class T = long> struct R {};
int a = def(S{});
int b = z(X<int>{});
int c = S{}.mf();
S::M<> m;
R<> r;
