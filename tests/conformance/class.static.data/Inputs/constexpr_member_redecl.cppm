// The module for ../constexpr-member-redecl-module-run.cpp: the redundant
// out-of-class redeclaration of an exported class's `constexpr` member
// makes the interface unit emit it (`weak_odr`), although unused here.
export module constexpr_member_redecl;
export struct A { static constexpr int n = 3; };
constexpr int A::n;
