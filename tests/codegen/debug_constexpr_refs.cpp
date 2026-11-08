// Debug info (WP-814): references to constants: emitted as constant DIGlobalVariables.
//@ args: -g
namespace n {
constexpr int k = 5;
inline constexpr double d = 1.5;
} // namespace n
struct T {
  static constexpr int m = 2;
  enum E { X = 3 };
};
const long big = -9;
int h(T t) { return n::k + (int)n::d + T::m + T::X + t.X + (int)big; }
