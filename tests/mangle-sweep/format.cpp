#include <format>
#include <string>
#include <vector>
#include <chrono>
std::string fmt_sweep(int i, double d, const std::string& s) {
  std::string r = std::format("{} {:.2f} {:>8} {:#x}", i, d, s, i);
  r += std::format("{}", 'c');
  r += std::format("{} {}", true, (void*)nullptr);
  r += std::format("{:*^10}", std::string_view("x"));
  std::format_to(std::back_inserter(r), "{}", 42L);
  auto n = std::formatted_size("{}", 1u);
  r += std::vformat("{}", std::make_format_args(i));
  r += std::to_string(n);
  return r;
}
