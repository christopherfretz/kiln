static_assert(pfx::small_spin == 2997);
constexpr long big = pfx::spin(1000000);
constexpr int too_deep = pfx::deep(100000);
constexpr int past = pfx::read_past();
constexpr int by_zero = pfx::chain1(0);
constexpr int null_read = pfx::Holder{}.get();
static_assert(pfx::chain1(1) == 3);
int main() { return 0; }
