// [class.union.general], [class.default.ctor]/2-3, [class.dtor]/7-8
// (P3074R7, C++26): a union's defaulted default constructor and destructor
// are no longer deleted because a variant member's are non-trivial; with no
// default member initializer both are trivial, and the default constructor
// begins no member's lifetime. Anonymous unions in classes too, and a union
// whose members are all const (the old [class.default.ctor]/2.4 rule is
// gone). Neither clang 22 nor GCC 16.1 implements P3074 (spec/sema-class.md
// §4.2).
//@ skip(clang): clang 22 does not implement P3074 (trivial unions)
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct S {
  int *p;
  constexpr S(int v) : p(new int(v)) {}
  constexpr S(const S &o) : p(new int(*o.p)) {}
  constexpr ~S() { delete p; }
};
union U { S s; int i; };
static_assert(__is_constructible(U));
static_assert(__is_trivially_constructible(U));
static_assert(__is_trivially_destructible(U));
U u;

struct A { union { S s; S arr[4]; }; int n = 0; };
static_assert(__is_constructible(A));
static_assert(__is_trivially_destructible(A));
A a;

union C { const int c; };
static_assert(__is_constructible(C));
C c;

// A user-provided destructor of the union itself is still not trivial.
union D { S s; constexpr D() {} constexpr ~D() {} };
static_assert(!__is_trivially_destructible(D));

// A trivial member union stays trivial, so does a class of one.
struct E { U u; };
static_assert(__is_trivially_constructible(E));
static_assert(__is_trivially_destructible(E));
