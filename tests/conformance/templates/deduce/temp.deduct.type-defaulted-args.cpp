// [temp.deduct.type]/9, [temp.arg.general]/9: a dependent class template-id
// `V<T>` names `V<T, alloc<T>>`, so it deduces against `V<int>` (whose
// arguments were completed with the defaults) like the spelled-out form.
template <class T> struct alloc {};
template <class T, class A = alloc<T>> struct V { T x; };
template <class T> constexpr int g(const V<T>&) { return 1; }
template <class T> constexpr int h(V<T, alloc<T>>) { return 2; }
template <class T> constexpr int k(V<T, int>) { return 3; }
template <class T> constexpr int k(V<T>) { return 4; }
constexpr V<int> v{};
constexpr V<char, int> w{};
static_assert(g(v) == 1);
static_assert(h(v) == 2);
static_assert(k(w) == 3);
static_assert(k(v) == 4);
