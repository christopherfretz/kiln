#include <meta>
#include <vector>
#include <string>
#include <map>
#include <optional>
#include <variant>
#include <tuple>
#include <functional>
#include <algorithm>
#include <memory>
#include <array>
namespace pfx {
struct Point {
  int x;
  long y;
  void move(int);
};
enum class Color { red, green, blue };
struct Incomplete;
template <class T> consteval std::size_t count_members() {
  return std::meta::nonstatic_data_members_of(^^T, std::meta::access_context::unchecked()).size();
}
consteval std::meta::info point_info() { return ^^Point; }
inline constexpr std::meta::info color_info = ^^Color;
}
