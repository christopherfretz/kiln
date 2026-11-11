export module imgvs;
export namespace vs {
template <class T> inline constexpr int kind = 0;
template <class T> inline constexpr int kind<T *> = 1 + kind<T>;
template <class T, class U> inline constexpr bool same = false;
template <class T> inline constexpr bool same<T, T> = true;
template <> inline constexpr int kind<char> = 7;
}
