// [temp.res.general]/4 (P0634): a qualified name with a dependent
// qualifier is a type without `typename` in a type-only context: the
// decl-specifier-seq of a member-declaration (also before an
// operator-function-id, and in a friend declaration), of a namespace-scope
// declaration, of a parameter of a member function or of a function whose
// declarator-id is qualified, of a lambda or requires-expression parameter,
// of a constant template parameter; a new-type-id, a defining-type-id, a
// trailing return type, a type-parameter's default and a cast's type-id
// (WP-1495; found running GCC's C++ testsuite).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
template<class T> struct S {
  T::type operator+(int);
  T::type const operator-(int);
  friend T::type operator*(S, int) { return 0; }
  T::type f(T::type);
  auto g() -> T::type;
  static T::type sm;
  T::type *p = new T::type;
  using U = T::type;
  void k() { auto x = static_cast<T::type>(0); (void)x; }
};
template<class T> T::type S<T>::operator+(int) { return 0; }
template<class T> T::type S<T>::f(T::type) { return 0; }
template<class T> T::type S<T>::sm;
template<class T, class U = T::type> struct D {};
template<class T, T::type N> struct E {};
template<class T> auto lam = [](T::type x) { return x; };
template<class T> concept C = requires(T::type x) { x; };
template<class T> T::type nsf(int);
struct M { typedef int type; };
S<M> s;
int use() { return (s + 1) + s.f(2) + (s * 3); }
