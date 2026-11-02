// [class.default.ctor]/3, [class.dtor]/9, [dcl.constexpr]: in C++23 clang decides
// whether an explicitly-defaulted constexpr default constructor or destructor
// of a class with a direct virtual base is valid with a running flag over the
// bases in order, then the members (DefaultedDefaultConstructorIsConstexpr):
// a virtual base clears it, a non-virtual base or member without a constexpr
// one sets it again. Clang 19 and 22 agree.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:55 error "destructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 57:17 note "virtual base class declared here"
//@ diag: 57:29 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 57:17 note "virtual base class declared here"
//@ diag: 59:57 error "destructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 59:13 note "virtual base class declared here"
//@ diag: 59:31 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 59:13 note "virtual base class declared here"
//@ diag: 60:58 error "destructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 60:13 note "virtual base class declared here"
//@ diag: 60:32 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 60:13 note "virtual base class declared here"
//@ diag: 62:55 error "destructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 62:13 note "virtual base class declared here"
//@ diag: 62:29 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 62:13 note "virtual base class declared here"
//@ diag: 63:67 error "destructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 63:17 note "virtual base class declared here"
//@ diag: 63:29 note "virtual base class declared here"
//@ diag: 63:41 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 63:17 note "virtual base class declared here"
//@ diag: 63:29 note "virtual base class declared here"
//@ diag: 65:32 error "copy constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 65:14 note "virtual base class declared here"
//@ diag: 70:39 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 70:13 note "virtual base class declared here"
//@ diag: 72:32 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 72:13 note "virtual base class declared here"
//@ diag: 73:49 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 73:13 note "virtual base class declared here"
//@ diag: 76:30 error "destructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 76:13 note "virtual base class declared here"
//@ diag: 81:24 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 81:12 note "virtual base class declared here"
//@ diag: 82:24 error "destructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 82:12 note "virtual base class declared here"
struct NC { NC(); constexpr NC(int) {} ~NC(); };
struct CE { constexpr CE() {} constexpr ~CE() {} };
struct V { int v = 1; };
struct V2 { };
struct NB { NB(); ~NB(); };
struct NB2 { NB2(); constexpr ~NB2() {} };
struct W : virtual V { };

// Order of bases matters: a non-virtual base without a constexpr default
// constructor after the virtual base sets the flag again.
struct A1 : virtual V, NB { constexpr A1() = default; constexpr ~A1() = default; };
struct A2 : NB, virtual V { constexpr A2() = default; constexpr ~A2() = default; };
struct A3 : virtual V { NC m; constexpr A3() = default; constexpr ~A3() = default; };
struct A4 : virtual V { CE m; constexpr A4() = default; constexpr ~A4() = default; };
struct A5 : virtual V { int m; constexpr A5() = default; constexpr ~A5() = default; };
struct A6 : virtual V { NC m; CE n; constexpr A6() = default; constexpr ~A6() = default; };
struct A7 : virtual V, CE { constexpr A7() = default; constexpr ~A7() = default; };
struct A8 : NB, virtual V2, virtual V { constexpr A8() = default; constexpr ~A8() = default; };
struct A9 : virtual V { NC m; int k = 0; constexpr A9() = default; constexpr ~A9() = default; };
struct A10 : virtual V { NC m; constexpr A10(const A10&) = default; A10(); };

// Default member initializers are skipped for the constructor but not the
// destructor; arrays use their element type; references and anonymous unions
// leave the flag unchanged.
struct B1 : virtual V { NC m = NC(1); constexpr B1() = default; };
struct B2 : virtual V { NC m[2]; constexpr B2() = default; };
struct B3 : virtual V { NC &r; constexpr B3() = default; };
struct B4 : virtual V { union { int a; NC n; }; constexpr B4() = default; };
struct B5 : virtual V { NC m = NC(1); constexpr ~B5() = default; };
struct B6 : virtual V { NC m[2]; constexpr ~B6() = default; };
struct B7 : virtual V, NB2 { constexpr ~B7() = default; };
struct B8 : virtual V, NB2 { constexpr B8() = default; };

// Only direct virtual bases count.
struct Z : W { constexpr Z() = default; constexpr ~Z() = default; };
struct T : virtual V { constexpr T() = default; };
struct S : virtual V { constexpr ~S() = default; };
