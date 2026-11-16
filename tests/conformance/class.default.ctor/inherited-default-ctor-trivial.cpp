// [class.default.ctor]/1,3, CWG2799: a class that inherits a default
// constructor (`using A::A;`) has an implicit default constructor despite its
// user-declared constructors, trivial when the base's is. Found running
// GCC's C++ testsuite (WP-1467).
struct A { A() = default; };
struct B : A { using A::A; B(int); };
static_assert(__is_trivially_constructible(B), "");
struct C : A { using A::A; };
static_assert(__is_trivially_constructible(C), "");
struct D : A { using A::A; D(int); D() = default; };
static_assert(__is_trivially_constructible(D), "");
struct E { E() {} };
struct F : E { using E::E; F(int); };
static_assert(!__is_trivially_constructible(F), "");
static_assert(__is_constructible(F), "");
B b;
