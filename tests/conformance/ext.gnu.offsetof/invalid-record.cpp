// [support.types.layout]/1, [expr.const]/10: `offsetof` (`__builtin_offsetof`)
// through a record whose definition is invalid (an unknown member type, a
// duplicate member, a negative array bound) is not a constant expression,
// with no note, for any member; a constexpr object of such a record and
// its members stay silent (clang `RecordDecl::isInvalidDecl`).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:19 error "unknown type name 'Undef'"
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:15 error "constexpr variable 'o2' must be initialized by a constant expression"
//@ diag: 27:35 error "duplicate member 'bad'"
//@ diag: 27:24 note "previous declaration is here"
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:29 error "'b' declared as an array with a negative size"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
struct B { int x; Undef y; int z; static constexpr int k = 3; constexpr int get() const { return x; } };
static_assert(__builtin_offsetof(B, z) == 0);
static_assert(__builtin_offsetof(B, x) == 0);
constexpr int o2 = __builtin_offsetof(B, y);
constexpr B bb{};
constexpr int g = bb.get();
constexpr int g2 = bb.z;
constexpr int g3 = B{}.z;
static_assert(sizeof(B) == 1);
struct V { int a; void bad(); int bad; };
static_assert(__builtin_offsetof(V, a) == 0);
constexpr V vv{1};
static_assert(vv.a == 1);
struct W { int a = 1; int b[-1]; };
constexpr W ww{};
static_assert(ww.a == 1);
static_assert(__builtin_offsetof(W, a) == 0);
