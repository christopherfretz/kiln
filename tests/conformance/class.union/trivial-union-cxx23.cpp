// [class.default.ctor]/2, [class.dtor]/7 (C++23, before P3074R7): a
// variant member with a non-trivial default constructor or destructor
// deletes the union's defaulted default constructor and destructor, an
// anonymous union's too, and a union of const members only has a deleted
// default constructor. The C++26 rules are trivial-union-cxx26.cpp.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:3 error "call to implicitly-deleted default constructor of 'U'"
//@ diag: 16:13 note "default constructor of 'U' is implicitly deleted because field 's' has no default constructor"
struct S {
  int *p;
  constexpr S(int v) : p(new int(v)) {}
  constexpr ~S() { delete p; }
};
union U { S s; int i; };
static_assert(!__is_constructible(U));
static_assert(!__is_destructible(U));
union C { const int c; };
static_assert(!__is_constructible(C));
struct A { union { S s; }; int n = 0; };
static_assert(!__is_constructible(A));
U u;
