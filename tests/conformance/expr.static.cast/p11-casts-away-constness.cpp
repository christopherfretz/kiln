// [conv.ptr]/2, [basic.type.qualifier]/3: an array type is as
// cv-qualified as its elements, so "pointer to array of const T" converts
// to "pointer to const void" and not to "pointer to void", implicitly or by
// static_cast. [expr.static.cast]/2, /11: a downcast to a less
// cv-qualified pointee or referent casts away qualifiers.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-unused-value
//@ diag: 23:3 error "static_cast from 'const ary_t *' (aka 'const char *const (*)[2]') to 'void *' is not allowed"
//@ diag: 24:3 error "static_cast from 'cint2 *' (aka 'const int (*)[2]') to 'void *' is not allowed"
//@ diag: 25:3 error "static_cast from 'const int (*)[3]' to 'void *' is not allowed"
//@ diag: 26:9 error "cannot initialize a variable of type 'void *' with an lvalue of type 'cint2 *' (aka 'const int (*)[2]')"
//@ diag: 27:3 error "static_cast from 'const int (*)[3]' to 'void *' is not allowed"
//@ diag: 32:3 error "static_cast from 'const B *' to 'D *' casts away qualifiers"
//@ diag: 33:3 error "static_cast from 'const B' to 'D &' casts away qualifiers"
//@ diag: 34:3 error "static_cast from 'volatile B *' to 'const D *' casts away qualifiers"
//@ diag: 36:43 error "static_cast from 'const B *' to 'D *' casts away qualifiers"
//@ diag: 37:15 note "in instantiation of function template specialization 'dep<B>' requested here"
typedef char const *ary_t[2];
typedef const int cint2[2];
void arrays(ary_t const *p, cint2 *q, const int (*r)[3], const int (**pp)[3]) {
  static_cast<void *>(p);
  static_cast<void *>(q);
  static_cast<void *>(r);
  void *v = q;
  static_cast<void *>(*pp);
}
struct B {};
struct D : B {};
void classes(const B *b, const B &rb, volatile B *vb) {
  static_cast<D *>(b);
  static_cast<D &>(rb);
  static_cast<const D *>(vb);
}
template <class T> void dep(const T *b) { static_cast<D *>(b); }
template void dep(const B *);
