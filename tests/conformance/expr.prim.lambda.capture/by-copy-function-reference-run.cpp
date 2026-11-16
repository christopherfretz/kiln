// [expr.prim.lambda.capture]/10: an entity captured by copy that is a
// reference to a function becomes a member of type "lvalue reference to
// the function type"; a reference type is never incomplete
// ([basic.types.general]/5), so the capture is valid and calls the
// function. Found running GCC's C++ testsuite (kiln rejected the capture
// as being of incomplete type).
//@ expect: run
//@ std: c++11
//@ stdout: 7
#include <cstdio>

int seven() { return 7; }

int main() {
  int (&f)() = seven;
  auto g = [f] { return f(); };
  auto h = [=] { return f(); };
  std::printf("%d\n", g() == 7 && h() == 7 ? 7 : 0);
}
