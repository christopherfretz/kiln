// The module unit of module-image.cpp and module-replay.cpp.
export module refs_mod;
#define TWICE(x) ((x) + (x))
export namespace geo {
struct Pt {
  int x, y;
  int sum() const { return x + y; }
};
int norm(Pt p) { return p.sum(); }
template <class T> T twice(T v) { return TWICE(v); }
template <class T> int measure(T t) { return norm(t) + t.sum(); }
enum class Color { red, green };
inline Color pick() { return Color::green; }
} // namespace geo
export {
int top(int a, int b = 2);
int top(int a, int b) { return geo::twice(a) * b; }
}
// A leading `::` and a destructor's `~` are carried (WP-2326).
export inline void lead(geo::Pt *p) { p->::geo::Pt::~Pt(); }
// Operator entries with their closing tokens, and ud-suffixes, are
// carried (WP-2820).
export namespace geo {
struct Grid {
  int operator[](int i) const { return i; }
};
inline Pt operator+(Pt a, Pt b) { return {a.x + b.x, a.y + b.y}; }
inline int operator""_px(unsigned long long v) { return int(v); }
inline int cell(Grid g, Pt p) { return g[(p + p).x] + 3_px; }
} // namespace geo
// Dependent operators and a fold-expression's operator, their found sets
// and the rows the importer's instantiations record, are carried
// (WP-2910).
export namespace geo {
template <class T> T doubled(T t) { return t + t; }
template <class... T> auto summed(T... t) { return (t + ...); }
} // namespace geo
// New- and delete-expressions naming a class's own allocation and
// deallocation functions, `::new` / `::delete` with the `::` as lead, and
// a dependent `delete new T` the importer instantiates (WP-2911).
export namespace geo {
struct Pool {
  static void *operator new(decltype(sizeof 0));
  static void operator delete(void *);
};
inline void churn() {
  delete new Pool;
  ::delete ::new int;
}
template <class T> void cycle() { delete new T; }
} // namespace geo
