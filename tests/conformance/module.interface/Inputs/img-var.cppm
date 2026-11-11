export module imgv;
export namespace iv {
template <class T> constexpr int size = sizeof(T);
template <class T> constexpr T pi = T(3.14159);
template <class T, class U> constexpr bool same = false;
template <class T> inline constexpr bool is_int = __is_same(T, int);
template <int N> constexpr int sq = N * N;
template <class... Ts> constexpr int count = sizeof...(Ts);
template <class T> constexpr T zero{};
template <class T> T counter = T(1);
template <class T> constexpr auto twice = T(2) * 2;
}
