// [temp.spec.partial.order]/1, [temp.func.order]: of several matching
// partial specializations of a variable template, the most specialized one
// is used.
template <class T> constexpr int which = 0;
template <class T> constexpr int which<T *> = 1;
template <class T> constexpr int which<const T *> = 2;
template <class T> constexpr int which<T **> = 3;
static_assert(which<int> == 0);
static_assert(which<int *> == 1);
static_assert(which<const int *> == 2);
static_assert(which<int **> == 3);
static_assert(which<const int **> == 3);
