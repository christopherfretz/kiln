// [expr.const]/5.15 (C++11 to C++23, before P2738R1), [expr.static.cast]/14:
// a conversion from `void *` to a pointer to object type is never a core
// constant expression before C++26, whether written as a static_cast, a
// C-style cast or a functional cast, or reached through a function
// template; the note is at the cast (with "in C++ standards before C++2c"
// for a non-null operand, without it for a null one). A conversion to
// `void *` stays allowed. clang 19 and 22 agree.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:43 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 50:15 note "in call to 'f1(&x)'"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:43 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 51:15 note "in call to 'f2(&x)'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:43 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 52:15 note "in call to 'f3(&x)'"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:59 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 53:15 note "in call to 'g<int>(&x)'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:34 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 54:15 note "in call to 'f4(&s)'"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:43 note "cast from 'const void *' is not allowed in a constant expression"
//@ diag: 55:15 note "in call to 'f5(nullptr)'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:37 note "cast from 'void *' is not allowed in a constant expression"
//@ diag: 56:15 note "in call to 'f6(nullptr)'"
//@ diag: 57:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 42:43 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 57:19 note "in call to 'f1(&x)'"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:16 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
constexpr int x = 4;
struct S { int a; };
constexpr S s = {5};
typedef const void *CVP;
typedef const int *CIP;
constexpr int f1(const void *p) { return *static_cast<const int *>(p); }
constexpr int f2(const void *p) { return *(const int *)p; }
constexpr int f3(const void *p) { return *CIP(p); }
template <class T> constexpr T g(const void *p) { return *static_cast<const T *>(p); }
constexpr int f4(CVP p) { return static_cast<const S *>(p)->a; }
constexpr bool f5(const void *p) { return static_cast<const int *>(p) == nullptr; }
constexpr bool f6(void *p) { return static_cast<int *>(p) == nullptr; }
constexpr bool f7(const int *p) { return static_cast<const void *>(p) == p; }
static_assert(f1(&x) == 4, "");
static_assert(f2(&x) == 4, "");
static_assert(f3(&x) == 4, "");
static_assert(g<int>(&x) == 4, "");
static_assert(f4(&s) == 5, "");
static_assert(f5(nullptr), "");
static_assert(f6(nullptr), "");
constexpr int k = f1(&x);
static_assert(f7(&x), "");
static_assert(*static_cast<const int *>(static_cast<const void *>(&x)) == 4, "");
