// [temp.expl.spec]/1, /3: an explicit specialization of a class template
// is a separate class with its own members.
template <class T> struct limits {
  static constexpr bool specialized = false;
};
template <> struct limits<int> {
  static constexpr bool specialized = true;
  static constexpr int max() { return 2147483647; }
  static constexpr int digits = 31;
};
template <> struct limits<bool> {
  static constexpr bool specialized = true;
  static constexpr int digits = 1;
};
static_assert(!limits<long>::specialized);
static_assert(limits<int>::specialized && limits<int>::max() == 2147483647);
static_assert(limits<int>::digits == 31 && limits<bool>::digits == 1);
