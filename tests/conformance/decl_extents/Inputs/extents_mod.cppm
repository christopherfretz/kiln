// The module unit of module-image.cpp and module-replay.cpp.
export module extents_mod;
#define TWICE(x) ((x) + (x))
export namespace geo {
struct Pt {
  int x, y;
  int sum() const { return x + y; }
};
template <class T> T twice(T v) { return TWICE(v); }
enum class Color { red, green };
} // namespace geo
export {
int top(int a, int b = 2);
}
