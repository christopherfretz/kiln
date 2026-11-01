// Immediate invocations of a class whose constant is larger than the slot
// it initializes (clang 19 `AggExprEmitter::VisitConstantExpr` ->
// `CreateCoercedStore`): `B` is not POD for layout, so its data size (5)
// is smaller than its size (8), and a materialized temporary is a
// `MayOverlap` slot written through a temporary and a 5-byte memcpy. The
// constant itself is the literal struct `{ i32, i8 }` clang's
// `ConstantAggregateBuilder` builds, not B's packed IR type.
struct B {
  int a;
  char b;
  constexpr B(int x, char y) : a(x), b(y) {}
};
consteval B make(int x) { return B(x, 3); }

B global(1, 2);

int bind() {
  const B &r = make(7);
  return r.a;
}

int member() { return make(8).b; }

int local() {
  B l = make(9);
  constexpr B c(5, 6);
  B m = c;
  return l.a + m.b;
}
