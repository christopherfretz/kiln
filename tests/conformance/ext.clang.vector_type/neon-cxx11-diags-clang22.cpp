// neon-cxx11-diags.cpp at clang 22, which names the scope and has
// [[clang::ext_vector_type(N)]] (WP-809).
//@ expect: compile-fail
//@ args: --target=aarch64-linux-gnu
//@ requires: clang>=22
//@ strict-diags
//@ diag: 31:15 error "Neon vector size must be 64 or 128 bits"
//@ diag: 32:15 error "invalid vector element type 'int'"
//@ diag: 33:16 error "invalid vector element type 'bool'"
//@ diag: 34:15 error "'clang::neon_vector_type' attribute requires an integer constant"
//@ diag: 35:48 error "'clang::neon_vector_type' attribute requires an integer constant"
//@ diag: 36:47 error "invalid vector element type 'T'"
//@ diag: 40:19 error "'clang::neon_vector_type' attribute cannot be applied to a declaration"
//@ diag: 41:26 error "Neon vector size must be 64 or 128 bits"
//@ diag: 42:12 warning "applying attribute 'clang::neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 43:16 warning "applying attribute 'clang::neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 44:9 warning "applying attribute 'clang::neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 45:20 warning "applying attribute 'clang::neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 46:3 warning "applying attribute 'clang::neon_vector_type' to a declaration is deprecated; apply it to the type instead"
//@ diag: 51:15 warning "unknown attribute 'gnu::neon_vector_type' ignored"

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
