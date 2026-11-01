// [dcl.link]/6: two declarations of a function or variable with C
// language linkage and the same name declare the same entity, whatever
// namespace each is in, so lookup that finds both through two
// using-directives is not ambiguous and a definition in one namespace
// defines the entity the others declare.
//@ expect: run
//@ strict-diags
//@ exit: 0
namespace A { extern "C" int kiln_ec_f(int); extern "C" int kiln_ec_v; }
namespace B { extern "C" int kiln_ec_f(int); extern "C" int kiln_ec_v; }
using namespace A;
using namespace B;
namespace C { extern "C" int kiln_ec_f(int x) { return x + 1; } }
extern "C" int kiln_ec_v = 2;
int g() { return kiln_ec_f(1) + kiln_ec_v; }
int main() { return g() + A::kiln_ec_f(0) + B::kiln_ec_v == 7 ? 0 : 1; }
