// [temp.pre]/5, [dcl.pre]/5: a class template is declared with no
// declarator, so its class-specifier must name it. An unnamed one, in a
// class template, a class or a namespace, is diagnosed once at its
// class-key and declares nothing: the enclosing class template still
// instantiates and is complete.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 16:22 error "cannot declare a class template with no name"
//@ diag: 21:22 error "cannot declare a class template with no name"
//@ diag: 24:20 error "cannot declare a class template with no name"
//@ diag: 28:15 error "static assertion failed"
//@ diag: 28:31 note "expression evaluates to '1 == 2'"

template <class T> struct A {
  template <class U> struct { U u; };
  T x = 1;
};
A<int> a;
struct B {
  template <class U> union { U u; };
  int y;
};
template <class U> struct { U u; };

static_assert(sizeof(A<int>) == sizeof(int));
static_assert(sizeof(B) == sizeof(int));
static_assert(sizeof(A<char>) == 2);
