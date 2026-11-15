// [dcl.init.general]/6.3, [cpp.include]: default-initialized
// `std::meta::info` constants (the null reflection) read from a
// precompiled header are the null reflection in its user.
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: none
//@ pch: Inputs/pch_null_info.h
//@ strict-diags
static_assert(pch_null == info{} && pch_null_arr[1] == info{});
static_assert(pch_holder.m == pch_null && pch_holder.k == 7);
constexpr info again = pch_null;
static_assert(again == info());
int main() { return pch_holder.k == 7 && again == pch_null ? 0 : 1; }
