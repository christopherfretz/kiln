//@ expect: ast
//@ oracle: clang
// [temp.names]/5: a `template` keyword after a nested-name-specifier is kept
// as written in decltype types and expressions (clang's hasTemplateKeyword).
namespace N { template <class T> int v = 0; template <class T> int g(T) { return 0; } }
struct S { template <class T> static int w; template <class T> static int h(T) { return 1; } };
template <class T> auto f1(T) -> decltype(N::template v<T>);
template <class T> auto f2(T) -> decltype(T::template w<int>);
template <class T> auto f3(T) -> decltype(N::v<T>);
template <class T> auto f4(T) -> decltype(S::template w<T>);
template <class T> int b1(T t) { return N::template g<T>(t) + S::template h<T>(t); }
template <class T> int b2() { return N::template v<T> + T::template w<long>; }
// Substitution keeps the keyword on a name still unresolved and drops it
// once the name resolves (`N::v<S>`).
template <class T> struct X {
  template <class U> decltype(N::template v<U> + T::template w<U>) f(U);
  template <class U> decltype(N::template v<T>) g(U);
};
X<S> x;
