// [cpp.predefined]: with reflection turned off (it is on by default at
// C++26), kiln's `__cpp_consteval` stays at P2564R3's 202211L; with it,
// it is the working draft's 202606L (consteval-p4101.cpp). C++23 has
// 202211L too (p2493-probe.cpp checks the floor).
//@ expect: compile-ok
//@ std: c++26
//@ args: -fno-reflection
//@ skip(clang): -fno-reflection is kiln's flag
//@ strict-diags
static_assert(__cpp_consteval == 202211L);
