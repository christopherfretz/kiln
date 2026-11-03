// A function first referenced while a class parameter or return type
// is incomplete is declared `void ()` without attributes (clang
// `GetOrCreateLLVMFunction` of a type `isFuncTypeConvertible` rejects);
// later declarations keep it, calls use their own type, a definition
// replaces it at the end of the module.
struct I;
struct J;
struct L;
struct I *ip;
void takes(I);
I returns();
void takes_ref(I&);
void (*p1)(I) = takes;
auto p2 = &returns;
void (*p_ref)(I&) = takes_ref;
void takes_j(J, int);
void (*p3)(J, int) = takes_j;
void takes_l(L);
void (*p4)(L) = takes_l;
namespace N {
struct Outer {
  struct Inner;
};
void takes_inner(Outer::Inner);
void (*p5)(Outer::Inner) = takes_inner;
}
struct J { int x; };
struct L { long a, b, c; };
struct N::Outer::Inner { char c; };
void use_j() { takes_j(J{}, 2); takes_j(J{7}, 3); }
void takes_l(L l) { (void)l; }
struct A { void m(I); };
void (A::*pm)(I) = &A::m;
struct I { int i; };
void late() { void (*q)(I) = takes; (void)q; }
