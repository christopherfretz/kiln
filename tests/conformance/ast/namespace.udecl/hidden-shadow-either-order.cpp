//@ expect: ast
//@ ast-implicit
//@ ast-filter: Hs
//@ xfail(kiln): injected-class-names are not declared yet (spec/sema-class.md §2.2) and a shadow's target is not printed under it; the shadows listed match
//@ requires: clang>=22
// [namespace.udecl]/14: the dump shows no shadow of a base member the
// derived class's own member hides, in either declaration order
// (WP-3145), nor one an implicitly-declared assignment operator hides
// ([namespace.udecl]/4) and that is declared at the class's `}` with no
// use of it (WP-3146; namespace.udecl/p4-implicit-assignment-hides-run.cpp has the uses),
// as clang's dump: the record keeps the shadow, marked hidden (spec/ast.md
// §4.2), and the dump leaves it out. The same for inherited constructors.
struct HsB { void f(); void f(int); };
struct HsD : HsB { using HsB::f; void f(); };
struct HsE : HsB { void f(); using HsB::f; };
struct HsX;
struct HsA { HsA &operator=(const HsX &); };
struct HsX : HsA { using HsA::operator=; };
struct HsC { HsC(int); HsC(double); };
struct HsG : HsC { using HsC::HsC; HsG(int); };
struct HsH : HsC { HsH(int); using HsC::HsC; };
