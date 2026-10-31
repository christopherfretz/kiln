//@ expect: ast
//@ oracle: clang
// [temp.arg.template]/1, [temp.dep.type]/6: `X<T>::template apply` is a
// dependent template name used as a template template argument; it is looked
// up again in `X<int>` when the alias template is instantiated.
template <class T> struct X {
  template <class U> using apply = U *;
};
template <class T, template <class> class F> struct B { using type = F<T>; };
template <class T> using A = typename B<T, X<T>::template apply>::type;
A<int> p = nullptr;
