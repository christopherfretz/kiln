// Clang extension (C11 6.7.2.4 in C++): an `_Atomic ( T )` specifier or
// `_Atomic T` qualifier whose operand substitution makes invalid is
// diagnosed at the `_Atomic` of the template definition, with the
// instantiation note (clang `TransformAtomicType`; WP-461)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:3 error "_Atomic cannot be applied to array type 'int[2]'"
//@ diag: 25:11 note "in instantiation of template class 'S<int[2]>' requested here"
//@ diag: 23:3 error "_Atomic cannot be applied to array type 'int[2]'"
//@ diag: 22:3 error "_Atomic cannot be applied to qualified type 'const int'"
//@ diag: 26:14 note "in instantiation of template class 'S<const int>' requested here"
//@ diag: 23:3 error "_Atomic cannot be applied to qualified type 'const int'"
//@ diag: 28:10 error "_Atomic cannot be applied to reference type 'int &'"
//@ diag: 31:1 note "in instantiation of template class 'P<int &>' requested here"
//@ diag: 29:13 error "_Atomic cannot be applied to reference type 'int &'"
//@ diag: 32:55 error "_Atomic cannot be applied to array type 'int[3]'"
//@ diag: 33:19 note "in instantiation of function template specialization 'sz<int[3]>' requested here"
//@ diag: 34:48 error "_Atomic cannot be applied to atomic type '_Atomic(int)'"
//@ diag: 35:12 note "in instantiation of function template specialization 'cast<_Atomic(int)>' requested here"
template <class T> struct S {
  int pad;
  _Atomic(T) x;
  _Atomic T q;
};
S<int[2]> s1;
S<const int> s2;
template <class T> struct P {
  static _Atomic(T) *make();
  using U = _Atomic(T);
};
P<int &>::U *pu;
template <class T> unsigned long sz() { return sizeof(_Atomic(T)); }
unsigned long n = sz<int[3]>();
template <class T> void cast(void *p) { (void)(_Atomic(T) *)p; }
void h() { cast<_Atomic(int)>(0); }
template <class T> void ded(_Atomic(T) *p);
template <class T> int ded2(T, _Atomic(T) * = 0);
int k = ded2(1);
