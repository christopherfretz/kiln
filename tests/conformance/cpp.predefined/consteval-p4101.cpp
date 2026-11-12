// [cpp.predefined] (the working draft, P4101R1): `__cpp_consteval` is
// 202606L, the consteval-only value model. kiln defines it with that value
// when reflection is enabled (the model is part of it; the default at
// C++26); with -fno-reflection it stays at P2564R3's
// 202211L (consteval-p4101-no-reflection.cpp).
//@ expect: compile-ok
//@ std: c++26
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
static_assert(__cpp_consteval == 202606L);
