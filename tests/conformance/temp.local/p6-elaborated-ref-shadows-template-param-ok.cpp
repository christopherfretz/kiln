// [temp.local]/6 valid twin: an elaborated-type-specifier naming a class
// that is not a template parameter, a qualified one, and a class-scope
// member whose name is not a parameter's all compile (WP-1091).
//@ expect: compile-ok
struct T { int v = 1; };
namespace N { struct U { int w = 2; }; }
template<class U> struct A {
  struct T *p = nullptr;
  struct N::U *q = nullptr;
  int f() { struct T t; struct N::U u; return t.v + u.w; }
};
template<class X> int g() { struct T t; return t.v; }
int main() { return A<int>().f() + g<int>() - 4; }
