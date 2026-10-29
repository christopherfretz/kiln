// [temp.spec.partial.order]/1, [temp.func.order]/3: the most specialized
// of the matching partial specializations is used.
template <class T> struct which { static constexpr int v = 0; };
template <class T> struct which<T *> { static constexpr int v = 1; };
template <class T> struct which<const T *> { static constexpr int v = 2; };
template <class T> struct which<T *const> { static constexpr int v = 3; };
template <class T> struct which<const T *const> { static constexpr int v = 4; };
static_assert(which<int>::v == 0);
static_assert(which<int *>::v == 1);
static_assert(which<const int *>::v == 2);
static_assert(which<int *const>::v == 3);
static_assert(which<const int *const>::v == 4);
