// [expr.call]/7, [class.temporary]/4: discarded member calls on class
// prvalues beyond the one-call case (WP-774): a call returning a class
// with a non-trivial destructor, chained calls on a returned temporary
// (`M{}.a().b();`), a call through a returned reference, arguments
// that are themselves temporaries with non-trivial destructors, a
// nested call in an argument, and a discarded conditional. Each
// temporary is materialized once, in evaluation order, and destroyed
// in reverse at the end of the full-expression
// (spec/codegen.md "Discarded expressions").
struct D { D(); D(const D &); ~D(); int x; };
struct M {
  M(); M(const M &); ~M();
  D e(D, int) &&;
  M a();
  M &r();
  D b();
  int i(D, D);
};
M mk();
void f1() { M{}.a().b(); }
void f2() { M{}.r().b(); }
void f3() { M{}.i(D{}, D{}); }
void f4() { mk().e(D{}, 1); }
void f5() { M{}.a().a().e(D{}, M{}.i(D{}, D{})); }
void f6() { M{}.b(), M{}.b(); }
void f7(bool c) { c ? M{}.b() : D{}; }
struct T { int x, y; };
T mt();
void f8() { mt(), mt(); }
void f9() { T{}, mt(); }
