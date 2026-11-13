// Reflection state evaluated in the prefix and used by the main file
// (WP-1396): a class completed by define_aggregate in the prefix, static
// arrays and strings the prefix's evaluations made, reflections held by
// constants, and consteval code over them compiled in the prefix.
#include <meta>
#include <array>
#include <string_view>
#include <vector>
namespace pfx {
struct Config;
consteval {
  std::meta::define_aggregate(^^Config,
                              {std::meta::data_member_spec(^^int, {.name = "width"}),
                               std::meta::data_member_spec(^^double, {.name = "scale"}),
                               std::meta::data_member_spec(^^bool, {.name = "enabled"})});
}
struct Widget {
  int id;
  long size;
  char tag;
  constexpr int twice() const { return id * 2; }
};
enum class Mode { off, low, high, max };
template <class T> consteval auto members() {
  return std::define_static_array(
      std::meta::nonstatic_data_members_of(^^T, std::meta::access_context::unchecked()));
}
template <class T> consteval std::string_view name_of(std::size_t i) {
  return std::define_static_string(std::meta::identifier_of(members<T>()[i]));
}
inline constexpr auto widget_members = members<Widget>();
inline constexpr std::string_view config_second = name_of<Config>(1);
template <class E> consteval std::string_view enum_name(E e) {
  template for (constexpr auto x : std::define_static_array(std::meta::enumerators_of(^^E)))
    if ([:x:] == e) return std::meta::identifier_of(x);
  return "?";
}
inline constexpr std::string_view high_name = enum_name(Mode::high);
inline constexpr std::meta::info widget_size = std::meta::nonstatic_data_members_of(
    ^^Widget, std::meta::access_context::unchecked())[1];
template <class T> struct Box {
  T value;
};
consteval std::meta::info box_of(std::meta::info t) {
  return std::meta::substitute(^^Box, {t});
}
template <class T> consteval std::size_t sum_sizes() {
  std::size_t n = 0;
  for (auto m : std::meta::nonstatic_data_members_of(^^T, std::meta::access_context::unchecked()))
    n += std::meta::size_of(m);
  return n;
}
}  // namespace pfx
