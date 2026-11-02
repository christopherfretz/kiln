// [dcl.constexpr]/3-4 and [basic.types.general]/10 before C++26: a
// constexpr constructor, destructor or member function of a class with
// virtual bases is ill-formed (clang applies it to every non-static member
// function), an explicitly-defaulted constexpr default or copy constructor
// is ill-formed with clang's `err_incorrect_defaulted_constexpr_with_vb`,
// and a class with virtual bases is not a literal type. Every error notes
// each virtual base. P3533R2 (C++26) lifts all of these; see
// constexpr-virtual-base-cxx26.cpp.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:34 error "constexpr constructor not allowed in struct with virtual base class"
//@ diag: 31:12 note "virtual base class declared here"
//@ diag: 32:49 error "constexpr member function not allowed in struct with virtual base class"
//@ diag: 32:12 note "virtual base class declared here"
//@ diag: 33:24 error "default constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 33:12 note "virtual base class declared here"
//@ diag: 34:24 error "copy constructor cannot be 'constexpr' in a class with virtual base class"
//@ diag: 34:12 note "virtual base class declared here"
//@ diag: 35:33 error "constexpr member function not allowed in struct with virtual base class"
//@ diag: 31:12 note "virtual base class declared here"
//@ diag: 37:13 error "constexpr variable cannot have non-literal type 'const S'"
//@ diag: 36:8 note "struct with virtual base classes is not a literal type"
//@ diag: 31:12 note "virtual base class declared here"
//@ diag: 36:12 note "virtual base class declared here"
//@ diag: 36:23 note "virtual base class declared here"
//@ diag: 38:3 error "no matching constructor for initialization of 'G'"
//@ diag: 33:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 33:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
struct V { constexpr V(int x = 0) : v(x) {} int v; };
struct L : virtual V { constexpr L() : V(1) {} };
struct R : virtual V { R() = default; constexpr ~R() {} };
struct G : virtual V { constexpr G() = default; };
struct E : virtual V { constexpr E(const E &) = default; E(); };
struct D : L, R { constexpr int get() const { return v; } };
struct S : virtual L, virtual R {};
constexpr S s;
G g;
