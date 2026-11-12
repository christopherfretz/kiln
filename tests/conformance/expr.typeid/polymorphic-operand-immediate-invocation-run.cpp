// [expr.typeid]/3, [expr.const]/16,25: in the operand of `typeid` that is
// a glvalue of polymorphic class type (potentially evaluated), a call of a
// consteval function is an immediate invocation, evaluated at compile
// time; it is never called at run time. kiln builds the operand as an
// unevaluated operand, so the call is not wrapped as an immediate
// invocation, codegen emits a call to `cb`, which is never defined, and
// the link fails (clang re-transforms the operand). WP-1322 sibling.
//@ expect: run
//@ std: c++20
//@ args: -Wno-potentially-evaluated-expression
//@ stdout: 1
//@ xfail(kiln): WP-1322 follow-up, immediate invocations in a polymorphic typeid operand are not wrapped in a ConstantExpr (call to an undefined consteval function at link time)
#include <cstdio>
#include <typeinfo>
struct B { virtual ~B() {} };
struct D : B {};
D d;
consteval B &cb() { return d; }
int main() { std::printf("%d\n", typeid(cb()) == typeid(D)); }
