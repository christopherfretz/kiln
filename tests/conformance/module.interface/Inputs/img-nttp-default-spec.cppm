export module imgnds;
export namespace nds {
template <typename T> struct is_int {
  enum { value = 0 };
};
template <> struct is_int<int> {
  enum { value = 1 };
};
template <typename T, bool = is_int<T>::value> struct promote {
  typedef double type;
};
template <typename T> struct promote<T, false> {};
template <> struct promote<float> {
  typedef float type;
};
template <> struct promote<double> {
  typedef double type;
};
template <typename T, int N = sizeof(T) * 2> struct twice {
  static constexpr int value = N;
};
} // namespace nds
