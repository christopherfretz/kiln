// [expr.const] (GNU labels as values): `&&l` is a constant `void *`
// (clang's `AddrLabelExpr` lvalue base: an object of unknown value). It is
// not null and equals itself, but each `&&l` expression is its own address
// (clang 22); comparing two label addresses for order is unspecified, and
// reading through one, offsetting it past one object, casting it to an
// object pointer or to an integer is not a core constant expression.
// `__builtin_constant_p(&&l)` is false. A label address returned from a
// constexpr function is a constant (with -Wreturn-stack-address).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:18 error "constexpr variable 'lt' must be initialized by a constant expression"
//@ diag: 34:27 note "comparison between pointers to unrelated objects '&&&l' and '&&&m' has unspecified value"
//@ diag: 35:19 error "constexpr variable 'rd' must be initialized by a constant expression"
//@ diag: 35:24 note "read of object '&&l' whose value is not known"
//@ diag: 35:40 note "temporary created here"
//@ diag: 37:19 error "constexpr variable 's2' must be initialized by a constant expression"
//@ diag: 37:52 note "cannot refer to element 2 of non-array object in a constant expression"
//@ diag: 38:18 error "constexpr variable 'ip' must be initialized by a constant expression"
//@ diag: 38:23 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'void *' is not similar to the target type 'int'"
//@ diag: 39:23 error "constexpr variable 'll' must be initialized by a constant expression"
//@ diag: 39:28 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 51:10 warning "returning address of label, which is local"
void f() {
l:
m:
  constexpr void *q = &&l;
  constexpr void *q2 = q;
  static_assert(q == q2);
  static_assert(q != &&l);
  static_assert(&&l != nullptr);
  static_assert(!!&&l);
  static_assert(!__builtin_constant_p(&&l));
  constexpr bool lt = &&l < &&m;
  constexpr void *rd = *(void *const *)&&l;
  constexpr void *s1 = (void *)((void *const *)&&l + 1);
  constexpr void *s2 = (void *)((void *const *)&&l + 2);
  constexpr int *ip = static_cast<int *>(&&l);
  constexpr long long ll = (long long)&&l;
  (void)lt, (void)rd, (void)s1, (void)s2, (void)ip, (void)ll;
}
constexpr bool k() {
l:
  void *p = &&l;
  void *r = p;
  return p == r && p != nullptr;
}
static_assert(k());
constexpr void *g() {
l:
  return &&l;
}
constexpr void *gv = g();
