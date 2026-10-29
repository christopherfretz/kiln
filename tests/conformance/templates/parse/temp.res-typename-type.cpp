//@ expect: ast
//@ oracle: clang
// [temp.res.general]/3: `typename` names a member type of a dependent type
template <class T> typename T::type get(typename T::type x) { return x; }
template <class T> struct A { typedef typename T::value_type V; V v; };
template <class T> void f() { typename T::template rebind<int>::other *p = nullptr; }
