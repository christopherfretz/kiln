#include <cstdio>
static_assert(pfx::widget_members.size() == 3);
static_assert(std::meta::identifier_of(pfx::widget_members[2]) == "tag");
static_assert(pfx::config_second == "scale" && pfx::name_of<pfx::Config>(2) == "enabled");
static_assert(pfx::high_name == "high");
static_assert(pfx::enum_name(pfx::Mode::max) == "max");
static_assert(std::meta::identifier_of(pfx::widget_size) == "size");
static_assert(std::meta::type_of(pfx::widget_size) == ^^long);
using IntBox = [:pfx::box_of(^^int):];
static_assert(std::is_same_v<IntBox, pfx::Box<int>>);
static_assert(pfx::sum_sizes<pfx::Config>() == sizeof(int) + sizeof(double) + sizeof(bool));
static_assert(pfx::name_of<IntBox>(0) == "value");
constexpr pfx::Config cfg{.width = 3, .scale = 1.5, .enabled = true};
constexpr pfx::Widget w{.id = 21, .size = 4, .tag = 'x'};
static_assert(w.[:pfx::widget_size:] == 4 && w.twice() == 42);
int main() {
  std::printf("%d %g %d %.*s\n", cfg.width, cfg.scale, cfg.enabled,
              static_cast<int>(pfx::high_name.size()), pfx::high_name.data());
  return cfg.enabled ? 0 : 1;
}
