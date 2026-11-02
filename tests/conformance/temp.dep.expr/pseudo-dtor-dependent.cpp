//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:56 error "invalid application of 'sizeof' to an incomplete type 'void'"
//@ diag: 36:15 note "in instantiation of function template specialization 'use_sizeof<int>' requested here"
//@ diag: 32:47 error "cannot take the address of an rvalue of type 'void'"
//@ diag: 37:15 note "in instantiation of function template specialization 'use_addr<A>' requested here"
// [expr.call]/5, [temp.dep.expr]/1, [expr.prim.id.dtor]/2: a call of a
// pseudo-destructor or destructor through an object of dependent type
// (`p->~T()`, `t.~T()`) is a type-dependent expression: its type is
// `void` only once the template is instantiated. So at the definition
// `decltype` of it is a dependent type, and `&`, `sizeof`, arithmetic,
// a conversion to `int` and overload resolution on it are all deferred;
// they are diagnosed (or not) at instantiation, as clang does
// (`CallExpr` takes the dependence of its `CXXPseudoDestructorExpr`
// callee). Only the two instantiations below are ill-formed.
struct A { ~A(); };
void ov(int);

// Never instantiated: nothing here is diagnosed.
template <class T> void never(T *p, T t) {
  int a = p->~T();
  int *b = &t.~T();
  auto c = sizeof(p->~T()) + (p->~T() + 1);
  ov(p->~T());
  decltype(t.~T()) *q = 0;
  int d = q;
  (void)a, (void)b, (void)c, (void)d;
}

// Instantiated with a scalar and with a class.
template <class T> void use_sizeof(T *p) { (void)sizeof(p->~T()); }
template <class T> void use_addr(T t) { (void)&t.~T(); }
template <class T> using D = decltype(static_cast<T *>(nullptr)->~T());
static_assert(__is_same(D<int>, void));
static_assert(__is_same(D<A>, void));
template void use_sizeof<int>(int *);
template void use_addr<A>(A);
