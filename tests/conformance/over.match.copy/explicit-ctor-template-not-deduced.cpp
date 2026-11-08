// [over.match.copy]/1.1: copy-initialization considers only converting
// constructors, so a non-dependently explicit constructor template is not
// a candidate and is not deduced at all (clang
// AddTemplateOverloadCandidate). Its substitution would otherwise fail
// outside the immediate context (forming `const void&` in a member of
// `conv<void>`), as libc++'s explicit `basic_string(const T&)` does when a
// `void` expression is passed to a `std::string` parameter in a SFINAE
// test.
//@ strict-diags
template <class T> struct conv {
  using R = const T&;
  static constexpr bool value = true;
};
template <bool B, class T = int> struct en {};
template <class T> struct en<true, T> { using type = T; };
struct S {
  template <class T, typename en<conv<T>::value, int>::type = 0>
  explicit S(const T&);
};
template <class T> void accept(T);
template <class T> T make();
template <class T, class = decltype(accept<S>(make<T>()))> int test(int);
template <class T> char test(...);
static_assert(sizeof(test<void>(0)) == 1);
static_assert(sizeof(test<S>(0)) == sizeof(int));
