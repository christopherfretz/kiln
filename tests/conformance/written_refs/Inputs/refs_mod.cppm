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
