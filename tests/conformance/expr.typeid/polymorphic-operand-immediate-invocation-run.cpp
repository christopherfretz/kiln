// [expr.typeid]/3, [expr.const]/16,25: in the operand of `typeid` that is
// a glvalue of polymorphic class type (potentially evaluated), a call of a
// consteval function is an immediate invocation, evaluated at compile
// time; it is never called at run time (`cb` is never emitted, so a call
// would not link). The operand is built as an unevaluated operand; its
// immediate invocations are wrapped all the same (clang re-transforms the
// operand). WP-1322 sibling, WP-1236.
//@ expect: run
//@ std: c++20
//@ args: -Wno-potentially-evaluated-expression
//@ stdout: 1
#include <cstdio>
#include <typeinfo>
struct B { virtual ~B() {} };
struct D : B {};
D d;
consteval B &cb() { return d; }
int main() { std::printf("%d\n", typeid(cb()) == typeid(D)); }
