// [temp.deduct.call]/4.3: a derived class deduces against its base template.
template <class T> struct base { T v; };
struct derived : base<int> {};
template <class T> constexpr T get(const base<T>& b) { return b.v; }
constexpr derived d{{6}};
static_assert(get(d) == 6);
