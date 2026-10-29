// [temp.spec.partial]/1, [temp.spec.partial.match]/1: a partial
// specialization of a variable template is used when it matches.
template <class T> constexpr bool is_ptr = false;
template <class T> constexpr bool is_ptr<T *> = true;
template <class T> constexpr bool is_ref = false;
template <class T> constexpr bool is_ref<T &> = true;
template <class T> constexpr bool is_ref<T &&> = true;
static_assert(!is_ptr<int>);
static_assert(is_ptr<int *>);
static_assert(is_ptr<const char *>);
static_assert(is_ptr<int **>);
static_assert(!is_ptr<int[3]>);
static_assert(is_ref<int &> && is_ref<int &&> && !is_ref<int>);
