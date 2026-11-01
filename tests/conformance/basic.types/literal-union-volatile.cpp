// [basic.types.general]/10: a union is a literal type if it has no data
// member or at least one non-volatile data member of literal type; any other
// class needs every data member to be non-volatile and of literal type.
// (clang `CXXRecordDecl::isLiteral`; the notes are `RequireLiteralType`'s.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:14 error "constexpr variable cannot have non-literal type 'const U1'"
//@ diag: 14:15 note "'U1' is not literal because it has data member 'n' of non-literal type 'NL'"
//@ diag: 17:14 error "constexpr variable cannot have non-literal type 'const U3'"
//@ diag: 16:25 note "'U3' is not literal because it has data member 'v' of volatile type 'volatile int'"
//@ diag: 19:13 error "constexpr variable cannot have non-literal type 'const W'"
//@ diag: 18:22 note "'W' is not literal because it has data member 'u' of non-literal type 'U1'"
struct NL { NL(); };
union U1 { NL n; constexpr U1() {} };
constexpr U1 u1;
union U3 { volatile int v; };
constexpr U3 u3{};
struct W { int a; U1 u; };
constexpr W w{};
struct E {};
union U6 { E e; NL n; constexpr U6() : e() {} };
static_assert(__is_literal_type(U6));
constexpr U6 u6;
union U7 { };
static_assert(__is_literal_type(U7));
struct R { int& r; NL* p; };
static_assert(__is_literal_type(R));
struct VA { volatile int a[2]; };
static_assert(!__is_literal_type(VA));
static_assert(!__is_literal_type(U1) && !__is_literal_type(U3) && !__is_literal_type(W));
union U8 { volatile int v; int w; };
static_assert(__is_literal_type(U8));
