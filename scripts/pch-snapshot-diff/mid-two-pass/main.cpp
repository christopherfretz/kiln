struct Packed {
  char c;
  int i;
};
static_assert(sizeof(Packed) == 6, "#pragma pack(2) from the prefix");
#pragma pack(pop)
#include <vector>
#include <map>
static int unused_helper() { return 1; }
constexpr int k = pfx::square(9);
static_assert(k == 81);
static_assert(pfx::Box<long>{5}.get() == 5);
static_assert(pfx::twice(21) == 42);
static_assert(pfx::small_size<short>() == 2);
static_assert(pfx::table[4] == 8 && PFX_TWICE(4) == 8);
template <class T> struct Local : pfx::Box<T> {};
int use_all(int n) {
  pfx::Widget w;
  pfx::Widget w2 = w;
  w2 = static_cast<pfx::Widget &&>(w);
  std::vector<pfx::Widget> ws;
  ws.push_back(w2);
  std::vector<int> v(n, 3);
  std::sort(v.begin(), v.end());
  std::map<std::string, int> m;
  m["a"] = touch(pfx::Tag{});
  int d = pfx::old_square(3);
  auto f = [&](int x) { return x + d + Local<int>{{n}}.get(); };
  ++pfx::counter;
  return pfx::declared_only(f(d)) + pfx::used_later() + static_cast<int>(ws.size()) + m["a"] +
         (ws[0] == w2);
}
