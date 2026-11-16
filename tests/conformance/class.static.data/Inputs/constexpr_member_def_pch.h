// The header for ../constexpr-member-def-pch-cxx14-run.cpp, precompiled:
// the out-of-class definition of a `constexpr` static data member.
struct PchA { static constexpr int n = 3; };
constexpr int PchA::n;
