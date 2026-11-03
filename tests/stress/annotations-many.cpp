// [dcl.attr.annotation]/2, spec/annotations.md §8: 10,000 annotated
// variables, each with an int annotation and a class-type annotation (a
// distinct template parameter object per value), expanded from nested
// macros so the source stays small. Every annotation is one constant
// evaluation (EvalKind::TemplateArg) and one frozen value; the budgets
// pin that the cost is linear and step-free: 20,000 evaluations, no Oven
// steps or calls, and the class values' frozen bytes. The brief's
// `refl_bytes` counter does not exist; `frozen_bytes` (the class-type
// values) and `static_bytes` (nothing reaches the static space) stand in
// for it.
//@ expect: compile-ok
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ oven: evaluations<=20000 steps<=0 calls<=0 allocs<=0 peak_bytes<=16 frozen_bytes<=480000 static_bytes<=0
//@ mem<=256
struct Tag { int id; };
#define D(n) [[=Tag{n}]] [[=n]] int v##n;
#define A(p) D(p##0) D(p##1) D(p##2) D(p##3) D(p##4) D(p##5) D(p##6) D(p##7) D(p##8) D(p##9)
#define B(p) A(p##0) A(p##1) A(p##2) A(p##3) A(p##4) A(p##5) A(p##6) A(p##7) A(p##8) A(p##9)
#define C(p) B(p##0) B(p##1) B(p##2) B(p##3) B(p##4) B(p##5) B(p##6) B(p##7) B(p##8) B(p##9)
#define E(p) C(p##0) C(p##1) C(p##2) C(p##3) C(p##4) C(p##5) C(p##6) C(p##7) C(p##8) C(p##9)
E(1)
int main() { return v10000 + v19999; }
