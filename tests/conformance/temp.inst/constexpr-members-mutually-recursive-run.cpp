// [temp.inst]/5, [temp.point]: constexpr member functions of a class
// template specialization are instantiated at their use, and two that
// use each other instantiate each once: the second use finds the first
// instantiation in progress (a move constructor calling a `swap` that
// moves `*this`, a type-erased value's `__value_proxy_root`; WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 0 3 5 3
extern "C" int printf(const char *, ...);

template <class T> constexpr T &&mv(T &t) noexcept { return static_cast<T &&>(t); }

template <class T> struct box {
  int v = 0;
  constexpr box() noexcept {}
  constexpr box(int x) noexcept : v(x) {}
  constexpr box(box &&o) noexcept : box() { swap(o); }
  constexpr void swap(box &o) noexcept {
    if (v == 0 || o.v == 0) {
      int t = v;
      v = o.v;
      o.v = t;
      return;
    }
    auto tmp = mv(*this);
    v = o.v;
    o.v = tmp.v;
  }
  constexpr int even(int n) const { return n == 0 ? 1 : odd(n - 1); }
  constexpr int odd(int n) const { return n == 0 ? 0 : even(n - 1); }
};

constexpr int moved() {
  box<int> a(3);
  box<int> b(mv(a));
  return b.v;
}
static_assert(moved() == 3);

int main() {
  box<int> a(3), b(5);
  b.swap(a);
  box<int> c(mv(b));
  printf("%d %d %d %d\n", b.v, c.v, a.v, moved() + box<int>().odd(4));
}
