export module imga;
export namespace ia {
template <class T> struct Box { T v; constexpr T get() const { return v; } };
template <class T> using Ptr = T *;
template <class T> using BoxOf = Box<T>;
template <class T> using Arr3 = T[3];
template <class... Ts> using First = int;
using IntBox = BoxOf<int>;
template <class T> constexpr T deref(Ptr<T> p) { return *p; }
template <class T> constexpr T unbox(BoxOf<T> b) { return b.get(); }
template <class T> using Fn = T (*)(T);
template <class T> constexpr T apply(Fn<T> f, T x) { return f(x); }
constexpr int twice(int x) { return 2 * x; }
}
