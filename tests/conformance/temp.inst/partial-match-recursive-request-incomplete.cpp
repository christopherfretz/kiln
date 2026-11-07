// [temp.inst]/2, [temp.spec.partial.match]: matching the partial
// specializations of `viewable<int>` checks `is_convertible<const int&,
// const str<char>&>`, whose overload resolution substitutes the default
// template argument `typename viewable<T>::type` of `str`'s constructor
// template with `T = int`: a request for `viewable<int>` itself while its
// pattern is being selected. As in clang the nested request finds it
// incomplete (a substitution failure), the outer selection completes once
// (Boost.StaticString `enable_if_viewable`; kiln panicked).
//@ expect: run
//@ stdout: 1
//@ stdout: 1
extern "C" int printf(const char*, ...);
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T> T&& declval();
template <class From, class To> struct is_convertible {
  template <class U> static void sink(U);
  template <class F, class T, class = decltype(sink<T>(declval<F>()))>
  static char test(int);
  template <class, class> static long test(...);
  static constexpr bool value = sizeof(test<From, To>(0)) == 1;
};
template <class T> struct str;
template <class T, class = void> struct viewable {};
template <class T>
struct viewable<T, typename enable_if<!is_convertible<const T&, const str<char>&>::value>::type> {
  using type = void;
};
template <class C> struct str {
  str() {}
  template <class T, class = typename viewable<T>::type> str(const T&) {}
  int size() const { return 1; }
};
int main() {
  printf("%d\n", (int)sizeof(viewable<int>));
  str<char> s(3);
  printf("%d\n", s.size());
}
