// [expr.const]/7 (constant destruction), [basic.stc.static] + kiln Oven
// budgets (spec/oven.md "Memory", WP-060): checking that each `sN` has
// constant destruction runs its destructor on a scratch static copy of
// the object, and the static object the destructor makes for `ext` comes
// after that copy, so the copy is not the most recent static when it
// dies. Every dead copy is reclaimed: the static space holds the 32 live
// 256-byte objects, their 8-byte `ext` objects and at most one copy
// (8960 bytes), where keeping each dead copy would take 16896.
//@ expect: run
//@ oven: static_bytes<=9216
int ext;
struct S {
  int arr[64] = {};
  constexpr ~S() {
    int *q = &ext;
    arr[0] = q != nullptr;
  }
};
constexpr S s0{};
constexpr S s1{};
constexpr S s2{};
constexpr S s3{};
constexpr S s4{};
constexpr S s5{};
constexpr S s6{};
constexpr S s7{};
constexpr S s8{};
constexpr S s9{};
constexpr S s10{};
constexpr S s11{};
constexpr S s12{};
constexpr S s13{};
constexpr S s14{};
constexpr S s15{};
constexpr S s16{};
constexpr S s17{};
constexpr S s18{};
constexpr S s19{};
constexpr S s20{};
constexpr S s21{};
constexpr S s22{};
constexpr S s23{};
constexpr S s24{};
constexpr S s25{};
constexpr S s26{};
constexpr S s27{};
constexpr S s28{};
constexpr S s29{};
constexpr S s30{};
constexpr S s31{};
int main() { return s0.arr[0] + s31.arr[63]; }
