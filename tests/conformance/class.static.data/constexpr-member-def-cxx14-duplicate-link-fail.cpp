// [basic.def.odr]/1 in C++14: before C++17 a `constexpr` static data
// member is not inline, so its out-of-class definition is a strong
// external definition; a second one in another unit makes the program
// fail to link (WP-1517, WP-1523).
//@ expect: run
//@ std: c++14
//@ strict-diags
//@ unit: Inputs/constexpr-member-def-cxx14-def.cpp
//@ link: fail
struct A { static constexpr int n = 3; };
constexpr int A::n;
int other();
int main() { return other() - A::n; }
