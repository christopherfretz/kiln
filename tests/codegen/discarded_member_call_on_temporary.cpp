// [expr.call]/7, [class.temporary]/2: a discarded member call on a
// temporary object whose result is a class prvalue (`M{}.g(3);`)
// materializes the object, then the arguments, then the result
// (clang `AggExprEmitter::VisitExprWithCleanups` keeps the ignored
// slot, so the only result temporary is `EmitCall`'s, allocated after
// the object's and the arguments'); no extra result temporary
// (spec/codegen.md "Discarded expressions").
struct E {};
struct D { D(); ~D(); };
struct M { E e(E, int) &&; E g(int) &&; D d(D) &&; ~M(); };
struct N { E g(int) const; };
void f1() { M{}.e(E{}, 3); }
void f2() { M().g(3); }
void f3() { N{}.g(3); }
void f4() { M{}.d(D{}); }
void f5() { (void)M{}.g(3); }
E f6() { return M{}.g(3); }
