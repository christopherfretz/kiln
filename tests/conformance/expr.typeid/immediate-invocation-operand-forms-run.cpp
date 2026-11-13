// [expr.typeid]/3, [expr.const]/16: immediate invocations nested in a
// polymorphic `typeid` operand are evaluated at compile time; in a
// non-polymorphic (unevaluated) operand they are not evaluated at all, so a
// consteval call that would fail to be a constant expression is fine there
// (clang 19 and 22 accept it). In a template the operand is substituted.
//@ expect: run
//@ std: c++20
//@ args: -Wno-potentially-evaluated-expression
//@ stdout: 1 1 1 1
#include <cstdio>
#include <typeinfo>
struct B { virtual ~B() {} };
struct D : B {};
D d;
consteval B &cb() { return d; }
consteval B *pb() { return &d; }
consteval int bad(int n) { return n ? throw 0 : 1; }
struct N { int x; };
template <class T> bool same() { return typeid(cb()) == typeid(T); }
int main() {
  bool a = typeid(*pb()) == typeid(D);
  bool b = typeid((cb())) == typeid(D);
  bool c = typeid(N{bad(1)}) == typeid(N);
  std::printf("%d %d %d %d\n", a, b, c, same<D>());
}
