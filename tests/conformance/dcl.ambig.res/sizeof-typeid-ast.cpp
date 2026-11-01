// [dcl.ambig.res]/2: in sizeof and typeid, an operand that can be a
// type-id is one (`T(*)()`, `T()`, `T(I)` with `I` a type); otherwise it
// is an expression (`T(a)` with `a` a variable). `sizeof(T())` is a
// function type: see type-id-errors.cpp.
//@ expect: ast
namespace std { class type_info; }
struct T { T(); T(int); int m; };
int a;
typedef int I;
unsigned long z1 = sizeof(T(a));
unsigned long z3 = sizeof(T(*)());
unsigned long z4 = sizeof(T(a).m);
unsigned long z5 = sizeof(int(a));
const std::type_info &t1 = typeid(T(a));
const std::type_info &t2 = typeid(T());
const std::type_info &t3 = typeid(T(I));
const std::type_info &t4 = typeid(T(*)(I));
const std::type_info &t5 = typeid(int(a) + 1);
