// The definition for ../constexpr-member-def-cxx14-two-units-run.cpp: the
// out-of-class definition, not odr-used in this unit (`other` reads the
// in-class initializer, a constant: no odr-use of `A::n`).
struct A { static constexpr int n = 3; };
constexpr int A::n;
int other() { return A::n; }
