// The prefix of subst-partial-pch.cpp and subst-partial-pch-include.cpp:
// a partial specialization matched once in the prefix.
namespace sp {
struct S {};
void swap(S &, S &);
struct V {};
void swap(V &, V &);
} // namespace sp
template <class T> T &ref();
template <class T, class = void> struct can_swap {
  static const bool value = false;
};
template <class T> struct can_swap<T, decltype(swap(ref<T>(), ref<T>()))> {
  static const bool value = true;
};
inline bool in_prefix = can_swap<sp::S>::value;
