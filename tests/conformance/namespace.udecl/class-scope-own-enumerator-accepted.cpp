// [basic.scope.scope]/6, [class.mem.general]/5, [namespace.udecl]/8: in a
// class, a using-declarator naming an enumerator of the class's own member
// enumeration names the member the class already has (the same entity):
// no conflict and no second member declaration. clang 22 accepts; GCC 16
// rejects (docs/status/deviations.md).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++20

struct Flags {
  enum Bit { read, write };
  enum class Mode { fast };
  using Bit::read;
  using Mode::fast;
  int pick() { return read + write + static_cast<int>(fast); }
};

struct All {
  enum Color { red, green };
  using enum Color;
  int sum() { return red + green; }
};

namespace scope {
  enum Kind { a };
  using Kind::a;
  using Kind::a;
}

static_assert(Flags::read == 0 && All::green == 1, "");
