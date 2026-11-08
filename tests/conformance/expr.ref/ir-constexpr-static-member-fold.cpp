// A class member access naming a constexpr static data member is emitted
// as the constant: clang `EmitMemberExpr` / `tryEmitAsConstant` evaluates
// the base for side effects only and never loads the variable, so it does
// not emit the variable either. kiln loads it from an emitted
// linkonce_odr global. Found by the WP-871 fmt smoke (fmt's
// `static_named_arg::name`, which also changed the UBSan check count).
//@ expect: ir
//@ std: c++20
//@ xfail(kiln): WP-871 follow-up, member access to a constexpr static data member is a load, not a constant
struct A {
  static constexpr const char *name = "x";
  static constexpr int n = 7;
  int m;
};

const char *by_ref(const A &a) { return a.name; }
const char *by_ptr(const A *p) { return p->name; }
int value(const A &a) { return a.n + a.m; }
