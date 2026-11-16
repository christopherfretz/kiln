// C++98 [class]/4: a POD-struct is an aggregate class with no non-POD
// members, no user-declared copy assignment and no user-declared
// destructor; `const` members do not matter, and C++98 has no deleted
// default constructor that would make the class non-trivial. Found
// running GCC's C++ testsuite.
//@ expect: run
//@ std: c++98
//@ stdout: 1
#include <cstdio>

struct P {
  const char *const a;
  const int b;
};

int main() { std::printf("%d\n", __is_pod(P) ? 1 : 0); }
