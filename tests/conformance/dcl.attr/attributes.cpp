//@ expect: compile-ok
[[nodiscard]] int must();
[[deprecated("old")]] void legacy();
[[maybe_unused]] static int spare;
alignas(16) int aligned_var;
struct alignas(8) Box { char c; };
int must() { [[maybe_unused]] int local = 1; return 0; }
