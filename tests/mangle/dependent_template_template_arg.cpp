// Mangling differential: a dependent member template named as a template
// template argument (`W<T::template X>`): its unresolved prefix and name
// (Itanium ABI <template-arg> ::= <type>; clang `mangleType(TemplateName)`
// for a `DependentTemplateName`: `mangleUnresolvedPrefix` then the source
// name), a substitution candidate shared with the dependent template
// prefix of `typename T::template X<int>`. It used to mangle as
// `u7unknown` (WP-1355 audit).
template <template <class> class TT> struct W {};
template <class T> struct H {
  template <class U> struct X {};
};
template <class T> struct Z {};
template <class T> void t1(W<H<T>::template X>) {}
template <class T> void t2(W<T::template X>) {}
template <class T>
void t3(W<T::template X>, W<T::template X>, typename T::template X<int>) {}
template <class T> void t4(typename H<T>::template X<int>, W<H<T>::template X>) {}
template <class T> void t5(W<H<T>::template X>, Z<H<T>>) {}

void use() {
  t1<int>({});
  t2<H<int>>({});
  t3<H<int>>({}, {}, {});
  t4<int>({}, {});
  t5<int>({}, {});
}
