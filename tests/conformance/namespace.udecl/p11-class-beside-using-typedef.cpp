// [namespace.udecl]/11 with [basic.scope.scope]/4: a class declared at
// namespace scope and a using-declarator that then brings in a
// typedef-name of the same name for another type correspond and declare
// different entities, so the text makes the program ill-formed; clang 22
// and GCC 16.1 accept both declarations, and kiln keeps them (a recorded
// deviation, docs/status/deviations.md). A typedef-name for the class
// itself names the same entity and is valid by the text too. Only the
// declarations are pinned: a later use of the name is ambiguous in clang
// and the typedef in GCC (and kiln). The reverse order (the
// using-declarator first, then the class) is rejected by clang and GCC.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++20
namespace N { typedef int X; typedef long Y; }
struct X { int v; };
using N::X;
namespace M {
  struct Y {};
  using N::Y;
}
struct Z {};
namespace P { typedef ::Z Z; }
using P::Z;
Z z;
