// __builtin_offsetof with an array index that is not a constant lowers as
// clang's ScalarExprEmitter::VisitOffsetOfExpr: one `add` per component
// (each base class, each member of an anonymous struct chain, each
// member, each index) once a run-time value is in the sum; an index is
// cast to size_t by its signedness and scaled with a plain `mul`. A
// constexpr function's call evaluated at compile time folds.
//@ expect: ir
//@ args: -Wno-invalid-offsetof
typedef __SIZE_TYPE__ size_t;
struct A { int pad[3]; };
struct B { long q; };
struct In { char c; struct { short u; int w[4]; }; };
struct D : A, B { int n; In in[5]; };
size_t f1(int i) { return __builtin_offsetof(D, in[i].w[i]); }
size_t f2(long i) { return __builtin_offsetof(D, in[2].w[i]); }
size_t f3(unsigned i) { return __builtin_offsetof(D, q) + __builtin_offsetof(D, in[i].u); }
constexpr size_t cx(int i) { return __builtin_offsetof(D, in[i].w[1]); }
size_t f4() {
  constexpr size_t v = cx(3);
  return v + cx(1);
}
size_t f5(int i) { return cx(i); }
// A negative constant index wraps the sum in size_t.
struct C { int n; char a[300]; };
size_t q3 = __builtin_offsetof(C, a[-5]);
size_t f6() { return __builtin_offsetof(C, a[-5]) + 1; }
