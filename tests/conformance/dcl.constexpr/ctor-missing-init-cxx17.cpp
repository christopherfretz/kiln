// [dcl.constexpr]/4 in C++17 (lifted by P1331R2): a constexpr constructor
// initializes every non-variant non-static data member, a union's exactly
// one variant member, and an anonymous union member's one member. clang
// 22 accepts a constructor that does not as a C++20 extension, with a
// note at each member left out (an anonymous struct or union at itself);
// GCC 16.1 rejects. A default member initializer, a member of class type
// (default-constructed) and an unnamed bit-field need no mem-initializer;
// a delegating constructor, a constructor of a class template and a
// constructor template are not checked, nor their instantiations. Found
// running GCC's C++ testsuite.
//@ args: -std=c++17
//@ strict-diags
//@ diag: 32:43 warning "constexpr constructor that does not initialize all members is a C++20 extension [-Wc++20-extensions]"
//@ diag: 32:20 note "member not initialized by constructor"
//@ diag: 34:38 warning "constexpr constructor that does not initialize all members is a C++20 extension [-Wc++20-extensions]"
//@ diag: 34:18 note "member not initialized by constructor"
//@ diag: 34:25 note "member not initialized by constructor"
//@ diag: 35:39 warning "constexpr union constructor that does not initialize any member is a C++20 extension [-Wc++20-extensions]"
//@ diag: 38:63 warning "constexpr constructor that does not initialize all members is a C++20 extension [-Wc++20-extensions]"
//@ diag: 38:20 note "member not initialized by constructor"
//@ diag: 41:64 warning "constexpr constructor that does not initialize all members is a C++20 extension [-Wc++20-extensions]"
//@ diag: 41:21 note "member not initialized by constructor"
//@ diag: 42:57 warning "constexpr constructor that does not initialize all members is a C++20 extension [-Wc++20-extensions]"
//@ diag: 42:41 note "member not initialized by constructor"
//@ diag: 43:79 warning "constexpr constructor that does not initialize all members is a C++20 extension [-Wc++20-extensions]"
//@ diag: 43:43 note "member not initialized by constructor"
//@ diag: 44:50 warning "constexpr constructor that does not initialize all members is a C++20 extension [-Wc++20-extensions]"
//@ diag: 44:30 note "member not initialized by constructor"

struct Empty {};
struct Class { Empty e; int b; constexpr Class() : b(0) {} };
struct Array { int a[2]; int b; constexpr Array() : b(0) {} };
struct Nsdmi { int a = 1; int b; constexpr Nsdmi() : b(0) {} };
struct Two { int a; int b; constexpr Two() {} };
union None { int a; char c; constexpr None() {} };
union One { int a; char c; constexpr One() : a(0) {} };
union Defaulted { int a = 0; char c; constexpr Defaulted() {} };
struct AnonUnion { union { int a; char c; }; int b; constexpr AnonUnion() : b(0) {} };
struct AnonInit { union { int a; char c; }; constexpr AnonInit() : a(0) {} };
struct Bits { int : 3; int b; constexpr Bits() : b(0) {} };
struct AnonStruct { struct { int p; int q; }; int b; constexpr AnonStruct() : b(0) {} };
struct PartStruct { struct { int p; int q; }; constexpr PartStruct() : p(0) {} };
struct Unions { union { int a; char c; }; union { int d; char e; }; constexpr Unions() : a(0) {} };
struct Mutable { mutable int a; int b; constexpr Mutable() : b(0) {} };
struct Delegating { int a; constexpr Delegating() : Delegating(1) {} constexpr Delegating(int x) : a(x) {} };
template <class T> struct Templ { T a; int b; constexpr Templ() : b(0) {} };
Templ<int> instance;
struct CtorTemplate { int a; template <class T> constexpr CtorTemplate(T) {} };
CtorTemplate from_template(1);
