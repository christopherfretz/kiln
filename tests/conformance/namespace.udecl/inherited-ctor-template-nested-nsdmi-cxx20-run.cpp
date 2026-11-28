// [namespace.udecl]/14, [temp.deduct.general]/8, [except.spec]/7: C++20
// variant of inherited-ctor-template-nested-nsdmi-cxx17-run.cpp (reduced from
// real code, regression since 1f2951a7b, WP-3685).
//@ expect: run
//@ std: c++20
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T> struct is_dc { static constexpr bool value = __is_constructible(T); };
template <class T1, class T2> struct pair {
  T1 first;
  T2 second;
  template <class U1 = T1, class U2 = T2,
            typename enable_if<is_dc<U1>::value && is_dc<U2>::value, bool>::type = true>
  constexpr pair() : first(), second() {}
};
template <class K, class V> struct DP : pair<K, V> { using pair<K, V>::pair; };
template <class K, class V> struct Map { char s[sizeof(DP<K, V>)]; };
struct M {
  struct Data { bool H = false; unsigned ID = 0; };
  Map<const int *, Data> Info;
};
int main() {
  M m;
  M::Data d;
  return (d.H || d.ID != 0) ? 1 : 0;
}
