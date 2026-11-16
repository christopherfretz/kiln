// The user for ../constexpr-member-def-pch-cxx14-run.cpp: declares the
// class without the out-of-class definition (C++14: the in-class
// declaration is not a definition) and odr-uses the member.
struct PchA { static constexpr int n = 3; };
const int *pch_user() { return &PchA::n; }
