// [expr.prim.id.dtor]/2, [basic.life]/1, [expr.const]/10: a
// pseudo-destructor call named through a simple-template-id
// (`p->~A<int>()`, `p->N::I::~B<int>()`, `p->A<int>::~A<int>()`) or in a
// template (`p->~A<T>()`, `p->T::I::~I()` instantiated with `T::I` = int)
// evaluates its object expression and ends the object's lifetime during
// constant evaluation: a later read or a second destruction is not a
// constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:75 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 50:15 note "in call to 'alias_read()'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:72 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 52:15 note "in call to 'prefix_read()'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:73 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 54:15 note "in call to 'first_read()'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:84 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 56:15 note "in call to 'dependent_read<int>()'"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:94 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 58:15 note "in call to 'dependent_qual_read<S>()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:63 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 60:15 note "in call to 'twice()'"
template <class T> using A = T;
namespace N {
using I = int;
template <class T> using B = T;
}
struct S { using I = int; };
union U { int i; };
constexpr int evaluated() {
  U u{1};
  int n = 0;
  [&] { ++n; return &u.i; }()->~A<int>();
  return n;
}
static_assert(evaluated() == 1);
template <class T> constexpr int dependent_ok() {
  U u{1}, v{2};
  (&u.i)->~A<T>();
  (&v.i)->T::~T();
  return 2;
}
static_assert(dependent_ok<int>() == 2);
constexpr int alias_read() { int i = 1; int* p = &i; p->~A<int>(); return *p; }
static_assert(alias_read() == 1);
constexpr int prefix_read() { int i = 1; (&i)->N::I::~B<int>(); return i; }
static_assert(prefix_read() == 1);
constexpr int first_read() { int i = 1; (&i)->A<int>::~A<int>(); return i; }
static_assert(first_read() == 1);
template <class T> constexpr int dependent_read() { T i = 1; (&i)->~A<T>(); return i; }
static_assert(dependent_read<int>() == 1);
template <class T> constexpr int dependent_qual_read() { int i = 1; (&i)->T::I::~I(); return i; }
static_assert(dependent_qual_read<S>() == 1);
constexpr int twice() { int i = 1; int* p = &i; p->~A<int>(); p->N::I::~B<int>(); return 0; }
static_assert(twice() == 0);
