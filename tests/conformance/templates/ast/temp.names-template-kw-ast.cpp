//@ expect: ast
//@ oracle: clang
// [temp.names]/5: 'T::template X<args>' resolved at instantiation.
template <class From> struct copy_cv { template <class To> using apply = const To; };
template <class T, class U> struct S { using type = typename T::template apply<U>; };
S<copy_cv<int>, long>::type v = 1;
