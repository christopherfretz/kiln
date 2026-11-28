// [namespace.udecl]/14, [temp.deduct.general]/8, [except.spec]/7: inheriting
// constructors from a base whose default constructor is a template with
// defaulted parameters (`enable_if<__is_constructible(...)>`) triggers an
// implicit-exception-specification evaluation on a nested class inside the
// enclosing class definition while checking whether the derived class's
// implicit default constructor is deleted. `SynthKind::ExceptionSpec`
// (clang `ExceptionSpecEvaluation`) is transparent to an enclosing SFINAE
// context, so the unavailable default member initializers of `M::Data` are a
// substitution failure inside `__is_constructible`, not a hard error
// (reduced from real code, regression since 1f2951a7b, WP-3685).
//@ expect: run
//@ std: c++17
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
