//@ expect: ast
//@ ast-implicit
//@ ast-filter: D::B
// [namespace.udecl]/1, [class.inhctor.init]/1, [class.default.ctor]/1:
// with implicit declarations, `using B::B;` declares one
// ConstructorUsingShadowDecl per base constructor (naming its target), and
// the inheriting constructor used to construct D is an implicit
// CXXConstructorDecl of D with the base constructor's signature. The base
// default constructor is not inherited, so D's own is declared. Matches
// clang 19 and clang 22 (whose JSON nests a shadow's target; the filter does not visit it).
struct B { B(int, int); B(double); B(); };
struct D : B { using B::B; };
void use() {
  D d(1, 2);
}
