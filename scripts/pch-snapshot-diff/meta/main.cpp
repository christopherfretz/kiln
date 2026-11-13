#include <meta>
#include <vector>
consteval std::size_t data_members() {
  return std::meta::nonstatic_data_members_of(^^pfx::Point,
                                              std::meta::access_context::unchecked())
      .size();
}
consteval bool third_is_function() {
  auto ms = std::meta::members_of(^^pfx::Point, std::meta::access_context::unchecked());
  return std::meta::is_function(ms[2]);
}
consteval std::string_view second_enumerator() {
  return std::meta::identifier_of(std::meta::enumerators_of(^^pfx::Color)[1]);
}
static_assert(std::meta::identifier_of(^^pfx::Point) == "Point");
static_assert(std::meta::identifier_of(pfx::point_info()) == "Point");
static_assert(pfx::color_info == ^^pfx::Color);
static_assert(std::meta::is_type(^^pfx::Point) && !std::meta::is_type(^^pfx::Point::x));
static_assert(data_members() == 2);
static_assert(pfx::count_members<pfx::Point>() == 2);
static_assert(third_is_function());
static_assert(second_enumerator() == "green");
consteval {
  std::meta::define_aggregate(^^pfx::Incomplete,
                              {std::meta::data_member_spec(^^int, {.name = "first"}),
                               std::meta::data_member_spec(^^long, {.name = "second"})});
}
static_assert(pfx::count_members<pfx::Incomplete>() == 2);
static_assert(std::meta::identifier_of(
                  std::meta::nonstatic_data_members_of(^^pfx::Incomplete,
                                                       std::meta::access_context::unchecked())[1]) ==
              "second");
struct Mine;
consteval {
  std::meta::define_aggregate(^^Mine, {std::meta::data_member_spec(^^pfx::Point, {.name = "p"})});
}
template <class T> constexpr auto names() {
  std::array<std::string_view, 2> out{};
  int i = 0;
  for (auto m : std::meta::nonstatic_data_members_of(^^T, std::meta::access_context::unchecked()))
    if (i < 2) out[i++] = std::meta::identifier_of(m);
  return out;
}
static_assert(names<pfx::Point>()[1] == "y");
std::optional<std::variant<int, std::string>> use_meta() { return std::string("x"); }
int use_mine() {
  Mine m{{1, 2}};
  pfx::Incomplete inc{3, 4};
  std::vector<pfx::Point> ps{m.p};
  return ps[0].x + static_cast<int>(inc.second);
}
