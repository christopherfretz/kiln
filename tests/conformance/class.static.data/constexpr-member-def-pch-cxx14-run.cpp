// [class.static.data]/3 in C++14 (N4140 9.4.2/3), [basic.def.odr]: the
// out-of-class definition of a `constexpr` static data member, in a
// precompiled header, is a strong definition that the object including
// the PCH emits although nothing there uses it, exactly as without the
// PCH; the other unit's odr-use links against it (WP-1517).
//@ expect: run
//@ std: c++14
//@ pch: Inputs/constexpr_member_def_pch.h
//@ unit: Inputs/constexpr-member-def-pch-user.cpp
const int *pch_user();
int main() { return *pch_user() == 3 ? 0 : 1; }
