// [dcl.constexpr]/6: a constexpr variable has a literal type. clang checks it
// at the declarator (or after deducing a placeholder, or at instantiation),
// before the initializer, with `RequireLiteralType`'s notes: virtual bases,
// then "no constexpr constructors", then a non-literal base, then a
// non-literal or volatile data member, then a non-constexpr destructor. The
// variable is invalid, so its initializer initializes nothing (no constructor
// is selected for it) but is still checked.
// Pinned to C++23: P3533R2 (C++26) allows virtual bases in constant evaluation.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:13 error "constexpr variable cannot have non-literal type 'const M'"
//@ diag: 46:8 note "struct with virtual base class is not a literal type"
//@ diag: 46:12 note "virtual base class declared here"
//@ diag: 48:13 error "constexpr variable cannot have non-literal type 'const M'"
//@ diag: 46:8 note "struct with virtual base class is not a literal type"
//@ diag: 46:12 note "virtual base class declared here"
//@ diag: 48:18 error "invalid operands to binary expression ('int' and 'std::nullptr_t')"
//@ diag: 51:13 error "constexpr variable cannot have non-literal type 'const X'"
//@ diag: 50:22 note "'X' is not literal because it has data member 'n' of non-literal type 'NL'"
//@ diag: 53:13 error "constexpr variable cannot have non-literal type 'const Y'"
//@ diag: 52:29 note "'Y' is not literal because its destructor is not constexpr"
//@ diag: 55:13 error "constexpr variable cannot have non-literal type 'const Z'"
//@ diag: 54:12 note "'Z' is not literal because it has base class 'X' of non-literal type"
//@ diag: 57:13 error "constexpr variable cannot have non-literal type 'const Q'"
//@ diag: 56:8 note "'Q' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 60:14 error "constexpr variable cannot have non-literal type 'const VF'"
//@ diag: 59:26 note "'VF' is not literal because it has data member 'a' of volatile type 'volatile int'"
//@ diag: 64:13 error "constexpr variable cannot have non-literal type 'const T'"
//@ diag: 63:15 note "'T' is not literal because it has data member 'n' of non-literal type 'NL[2]'"
//@ diag: 65:13 error "constexpr variable cannot have non-literal type 'const X[2]'"
//@ diag: 50:22 note "'X' is not literal because it has data member 'n' of non-literal type 'NL'"
//@ diag: 66:16 error "constexpr variable cannot have non-literal type 'const X'"
//@ diag: 50:22 note "'X' is not literal because it has data member 'n' of non-literal type 'NL'"
//@ diag: 67:49 error "constexpr variable cannot have non-literal type 'const X'"
//@ diag: 68:15 note "in instantiation of template class 'S<X>' requested here"
//@ diag: 50:22 note "'X' is not literal because it has data member 'n' of non-literal type 'NL'"
//@ diag: 69:24 error "constexpr variable cannot have non-literal type 'const X'"
//@ diag: 50:22 note "'X' is not literal because it has data member 'n' of non-literal type 'NL'"
//@ diag: 69:49 error "constexpr variable cannot have non-literal type 'const X'"
//@ diag: 50:22 note "'X' is not literal because it has data member 'n' of non-literal type 'NL'"
//@ diag: 70:42 error "constexpr variable cannot have non-literal type 'const X'"
//@ diag: 71:15 note "in instantiation of function template specialization 'g<X>' requested here"
//@ diag: 50:22 note "'X' is not literal because it has data member 'n' of non-literal type 'NL'"
struct V { constexpr V(int v) : v(v) {} int v; };
struct M : virtual V { using V::V; };
constexpr M m2("x");
constexpr M m3(1 + nullptr);
struct NL { NL(); };
struct X { int a; NL n; };
constexpr X x{};
struct Y { constexpr Y() {} ~Y(); };
constexpr Y y;
struct Z : X {};
constexpr Z z{};
struct Q { Q(int); Q(const Q&); };
constexpr Q q(1);
constexpr volatile int vi = 1;
struct VF { volatile int a; };
constexpr VF vf{1};
struct L { int a; };
constexpr L ok{1};
struct T { NL n[2]; };
constexpr T t{};
constexpr X arr[2] = {};
constexpr auto a = X{};
template<class U> struct S { static constexpr U m{}; };
const X& r2 = S<X>::m;
void f() { constexpr X lx{}; static constexpr X sx{}; }
template<class U> void g() { constexpr U gx{}; }
template void g<X>();
template void g<L>();
