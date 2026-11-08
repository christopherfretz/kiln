// [expr.sizeof]/1, [expr.alignof]/1, [temp.res.general]/6: a dependent
// operand type is checked when it is substituted, so `sizeof(void(T))`
// (a function type in every instantiation) is accepted in a template
// that is never instantiated, and `sizeof(T)` is diagnosed only in the
// instantiation that makes `T` a function type, with the instantiation
// note (clang `CreateUnaryExprOrTypeTraitExpr` checks only a
// non-dependent type). WP-821.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:34 error "invalid application of 'sizeof' to a function type"
//@ diag: 46:9 note "in instantiation of template class 'A<void ()>' requested here"
//@ diag: 33:48 error "invalid application of 'sizeof' to a function type"
//@ diag: 42:9 note "in instantiation of function template specialization 'sz<void ()>' requested here"
//@ diag: 34:48 error "invalid application of 'alignof' to a function type"
//@ diag: 43:9 note "in instantiation of function template specialization 'al<int (int)>' requested here"
//@ diag: 23:47 error "invalid application of 'sizeof' to a function type"
//@ diag: 44:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 37:54 error "invalid application of 'sizeof' to a function type"
//@ diag: 45:9 note "in instantiation of function template specialization 'se<int>' requested here"

// Never instantiated: accepted.
template <class T> auto fp() -> void (*)(T);
template <class T> unsigned long f() { return sizeof(void(T)); }
template <class T> unsigned long g() { return alignof(int(T)); }
template <class T> unsigned long g2() { return __alignof(int(T)); }
template <class... T> unsigned long h() { return sizeof(void(T...)); }
template <class T> unsigned long k() { return sizeof(T(int)); }
template <class T> unsigned long e() { return sizeof(*fp<T>()); }

template <class T> struct A {
  static const unsigned long n = sizeof(T);
};
template <class T> unsigned long sz() { return sizeof(T); }
template <class T> unsigned long al() { return alignof(T); }
template <class T> unsigned long ok() { return sizeof(T); }
template <class T> auto fq() -> void (*)(T);
template <class T> unsigned long se() { return sizeof(*fq<T>()); }

void use() {
  (void)ok<int>();
  (void)ok<void (*)()>();
  (void)sz<void()>();
  (void)al<int(int)>();
  (void)f<int>();
  (void)se<int>();
  (void)A<void()>::n;
}
