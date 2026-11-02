// [expr.static.cast]/2,11: a static_cast from a virtual base class, or
// from a base of a virtual base, down to a derived class is ill-formed,
// for references (lvalue and xvalue) and pointers, cv-qualified or not;
// dynamic_cast is the way down ([expr.dynamic.cast]/8). clang names the
// virtual base the path goes through. P3533R2 (C++26) leaves this as it
// is; the constant evaluator never sees such a cast.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:26 error "cannot cast 'V' to 'D &' via virtual base 'V'"
//@ diag: 21:26 error "cannot cast 'V *' to 'D *' via virtual base 'V'"
//@ diag: 22:26 error "cannot cast 'V *' to 'E *' via virtual base 'V'"
//@ diag: 23:34 error "cannot cast 'const V *' to 'const D *' via virtual base 'V'"
//@ diag: 24:24 error "cannot cast 'V' to 'D &&' via virtual base 'V'"
//@ diag: 29:33 error "cannot cast 'W' to 'X &' via virtual base 'V2'"
struct V { int v; virtual ~V(); };
struct L : virtual V {};
struct R : virtual V {};
struct D : L, R {};
struct E : D {};
D &by_ref(V &v) { return static_cast<D &>(v); }
D *by_ptr(V *v) { return static_cast<D *>(v); }
E *nested(V *v) { return static_cast<E *>(v); }
const D *cv(const V *v) { return static_cast<const D *>(v); }
D &&xv(V &&v) { return static_cast<D &&>(v); }
L *ok(V *v) { return dynamic_cast<L *>(v); }
struct W { int w; };
struct V2 : W {};
struct X : virtual V2 {};
X &base_of_vbase(W &w) { return static_cast<X &>(w); }
