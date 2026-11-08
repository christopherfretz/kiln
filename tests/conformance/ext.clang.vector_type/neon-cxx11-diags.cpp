// [[clang::neon_vector_type(N)]] and [[clang::neon_polyvector_type(N)]] (WP-809;
// clang Clang<"neon_vector_type">): the GNU spelling's type attribute; after a
// declarator clang slides it onto the declared type, with an error on a typedef
// and -Wdeprecated-attributes elsewhere. clang 19 has no [[clang::ext_vector_type]].
//@ expect: compile-fail
//@ args: --target=aarch64-linux-gnu
//@ requires: clang<22
//@ strict-diags
//@ diag: 34:15 error "Neon vector size must be 64 or 128 bits"
//@ diag: 35:15 error "invalid vector element type 'int'"
//@ diag: 36:16 error "invalid vector element type 'bool'"
//@ diag: 37:15 error "'neon_vector_type' attribute requires an integer constant"
//@ diag: 38:48 error "'neon_vector_type' attribute requires an integer constant"
//@ diag: 39:47 error "invalid vector element type 'T'"
//@ diag: 43:19 error "'neon_vector_type' attribute cannot be applied to a declaration"
//@ diag: 44:26 error "Neon vector size must be 64 or 128 bits"
//@ diag: 45:12 warning "applying attribute 'neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 46:16 warning "applying attribute 'neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 47:9 warning "applying attribute 'neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 48:20 warning "applying attribute 'neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 49:3 warning "applying attribute 'neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 54:15 warning "unknown attribute 'neon_vector_type' ignored"
//@ diag: 55:17 warning "unknown attribute 'ext_vector_type' ignored"

// Type position: the attribute modifies the type it follows.
typedef int [[clang::neon_vector_type(4)]] i4;
typedef unsigned char [[clang::neon_polyvector_type(8)]] p8;
using p16 = unsigned short [[clang::neon_polyvector_type(8)]];
static_assert(sizeof(i4) == 16 && sizeof(p8) == 8 && sizeof(p16) == 16);
static_assert(alignof(i4) == 16);
static_assert(sizeof(int [[clang::neon_vector_type(2)]]) == 8);

// The GNU spelling's checks.
typedef int [[clang::neon_vector_type(3)]] bad_size;
typedef int [[clang::neon_polyvector_type(4)]] bad_poly;
typedef bool [[clang::neon_vector_type(8)]] bad_elem;
typedef int [[clang::neon_vector_type(2.5)]] bad_count;
template <int N> using dependent_count = int [[clang::neon_vector_type(N)]];
template <class T> using dependent_elem = T [[clang::neon_vector_type(4)]];

// After a declarator: applied to the declared type, an error on a typedef
// (unless the attribute failed) and deprecated elsewhere.
typedef int i4d [[clang::neon_vector_type(4)]];
typedef int bad_size_d [[clang::neon_vector_type(3)]];
using u4 [[clang::neon_vector_type(4)]] = unsigned;
void f(int x [[clang::neon_vector_type(4)]]);
int v [[clang::neon_vector_type(4)]];
struct S { int m [[clang::neon_vector_type(4)]]; };
[[clang::neon_vector_type(2)]] int lead;
static_assert(sizeof(i4d) == 16 && sizeof(u4) == 16 && sizeof(v) == 16);
static_assert(sizeof(S) == 16 && sizeof(lead) == 8);

// Not a gnu:: attribute; clang:: ext_vector_type is clang 22's.
typedef int [[gnu::neon_vector_type(4)]] not_gnu;
typedef float [[clang::ext_vector_type(4)]] f4;
