// Interface unit of module `m` for the module.interface tests
// ([module.interface], [module.reach], [basic.lookup.argdep]/4).
module;
inline int gmf_helper(int x) { return x + 1; }
export module m;
export int answer();
export inline int twice(int x) { return 2 * x; }
export template <class T> constexpr T sq(T t) { return t * t; }
export struct Point {
  int x, y;
  int sum() const;
};
int Point::sum() const { return x + y; }
int secret() { return 40; }
int answer() { return secret() + 2; }
export namespace ns {
int counter = 7;
}
export constexpr int limit = 5;
namespace detail {
struct Hidden {
  int v = 3;
};
} // namespace detail
export detail::Hidden make_hidden() { return {}; }
// Not exported, but found by ADL from importers: same namespace as the
// exported Point, declared in m's interface ([basic.lookup.argdep]/4).
int frob(Point p) { return p.x * 10; }
export int use_gmf(int x) { return gmf_helper(x); }
