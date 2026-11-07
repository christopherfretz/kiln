// [temp.dep.candidate]/1, [temp.spec.partial.match]/2: an unqualified call
// with explicit template arguments (`declval<const T &>()`, `g<T>()`) in a
// template's definition keeps the declarations ordinary lookup found
// there. Substituting a partial specialization's arguments where the
// specialization is named, outside the template's namespace, must still
// find them, so the partial specialization matches (doctest's
// `is_std_string` SFINAE trait, WP-683). Matches clang 19.
//@ expect: run
//@ std: c++11
//@ exit: 0
namespace lib {
template <bool C, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T, class U> struct is_same { static constexpr bool value = false; };
template <class T> struct is_same<T, T> { static constexpr bool value = true; };
template <class T> T &&declval();
template <class T> int g();

template <class T, class E = void> struct HasCStr { static constexpr bool value = false; };
template <class T>
struct HasCStr<T, typename enable_if<
                      is_same<decltype(declval<const T &>().c_str()), const char *>::value>::type> {
  static constexpr bool value = true;
};

template <class T, class E = void> struct G { static constexpr int value = 0; };
template <class T> struct G<T *, decltype((void)g<T>())> { static constexpr int value = 1; };

struct String {
  template <class T, typename enable_if<HasCStr<T>::value, bool>::type = true>
  String(const T &s) : p(s.c_str()) {}
  const char *p;
};
} // namespace lib

namespace other {
struct Str {
  const char *c_str() const { return "ok"; }
};
} // namespace other

static_assert(lib::HasCStr<other::Str>::value, "");
static_assert(!lib::HasCStr<int>::value, "");
static_assert(lib::G<int *>::value == 1, "");
static_assert(lib::G<int>::value == 0, "");

int main() {
  other::Str s;
  lib::String t = lib::String(s);
  return t.p[0] == 'o' && t.p[1] == 'k' ? 0 : 1;
}
