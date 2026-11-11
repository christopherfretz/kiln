// The module of image-pack-run.cpp and image-pack-errors.cpp
// (spec/modules.md §11.11 "Templates as patterns", WP-1279): template
// parameter packs, `sizeof...`, fold-expressions, function parameter
// packs, `auto` placeholders in patterns and default arguments of
// non-type template parameters, all carried by the declaration image.
export module imgp;
export namespace ip {
template <class... Ts> constexpr int count() { return sizeof...(Ts); }
template <class... Ts> constexpr int count_args(Ts... xs) { return sizeof...(xs); }
template <class... Ts> constexpr auto sum(Ts... xs) { return (xs + ... + 0); }
template <class... Ts> constexpr auto lsum(Ts... xs) { return (0 + ... + xs); }
template <class... Ts> constexpr bool all(Ts... xs) { return (... && xs); }
template <class T> constexpr decltype(auto) inc(T x) { return x + 1; }
template <class T> constexpr int twice(T x) {
  auto y = x;
  return y + x;
}
template <int... Ns> constexpr int nsum() { return (Ns + ... + 0); }
template <int N = 3> constexpr int def() { return N * 2; }
template <class T, int N = sizeof(T)> constexpr int sz() { return N; }
template <class... Ts> struct Box {
  static constexpr int n = sizeof...(Ts);
  int size() const { return sizeof...(Ts); }
};
template <class T, int N = 4> struct Arr {
  T v{};
  constexpr int len() const { return N; }
};
template <class T, class... Rest> constexpr T first(T a, Rest...) { return a; }
} // namespace ip
