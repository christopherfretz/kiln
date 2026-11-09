// A class member access naming a constexpr static data member is emitted
// as the constant: clang `EmitMemberExpr` / `tryEmitAsConstant` evaluates
// the base for side effects only and never loads the variable, so it does
// not emit the variable either, for any scalar type (pointer, floating,
// member pointer). A non-static member of a constexpr object is folded only
// when integral (`EvaluateAsInt`). WP-940: kiln loaded a pointer from an
// emitted linkonce_odr global. Found by the WP-871 fmt smoke (fmt's
// `static_named_arg::name`, which also changed the UBSan check count).
//@ expect: ir
//@ std: c++20
struct B { int x; };
struct A {
  static constexpr const char *name = "x";
  static constexpr int n = 7;
  static constexpr double d = 1.5;
  static constexpr float f = 2.5f;
  static constexpr int B::*pm = &B::x;
  static const char *const cname;
  static constexpr const int *np = nullptr;
  const char *p;
  int m;
};
const char *const A::cname = "y";
constexpr A ca = {"z", 1};
A &get();

const char *by_ref(const A &a) { return a.name; }
const char *by_ptr(const A *p) { return p->name; }
int value(const A &a) { return a.n + a.m; }
const char *side_effect_base() { return get().name; }
double floating(const A &a) { return a.d; }
float floating_ptr(const A *a) { return a->f; }
int B::*member_pointer(const A &a) { return a.pm; }
const char *out_of_line_init(const A &a) { return a.cname; }
const int *null_pointer(const A &a) { return a.np; }
const char *pointer_field() { return ca.p; }
int integral_field() { return ca.m; }
const char *qualified() { return A::name; }
