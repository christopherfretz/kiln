#include <bits/stdc++.h>
namespace pfx {
struct Widget {
  int a = 1;
  std::string name;
  bool operator==(const Widget &) const = default;
};
template <class T> struct Box {
  T v;
  constexpr T get() const { return v; }
};
constexpr int square(int x) { return x * x; }
template <class T> constexpr T twice(T t) { return t + t; }
struct Tag {};
inline int touch(Tag) { return 7; }
int declared_only(int);
[[deprecated("use square")]] inline int old_square(int x) { return x * x; }
inline int used_later() { return 3; }
template <class T> concept Small = sizeof(T) <= 4;
template <Small T> constexpr int small_size() { return sizeof(T); }
constexpr int table[] = {1, 2, 3, 5, 8};
inline int counter = 0;
#define PFX_TWICE(x) ((x) + (x))
}
#pragma pack(push, 2)
