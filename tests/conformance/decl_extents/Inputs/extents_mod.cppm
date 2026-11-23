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
export namespace geo {
template <class T> struct Box {};
template <class T> struct Box<T *>;
template <class T> struct Box<T *> {};
enum class Opaque : int;
enum class Opaque : int;
template struct Box<int>;
template struct __attribute__((visibility("default"))) Box<long>;
template __attribute__((visibility("default"))) long twice<long>(long);
} // namespace geo
