// [dcl.align]/5: the combined effect of the alignment-specifiers of a
// declaration shall not specify an alignment less strict than its type's
// (clang `CheckAlignasUnderalignment`): checked for a complete variable,
// member or class definition (including one named only by a typedef or
// alias, a union, and a class template specialization, at the pattern's
// alignas with the instantiation note), against the largest requested
// alignment (alignas and GNU aligned), reported at the last alignas;
// alignas(0) is ignored and an inherited alignas counts. Also
// [dcl.align]/1: alignas cannot apply to an enumeration or a bit-field, and
// an attribute-specifier-seq after a bit-width cannot appear there.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 37:8 error "requested alignment is less than minimum alignment of 4 for type 'S1'"
//@ diag: 38:8 error "requested alignment is less than minimum alignment of 4 for type 'S2'"
//@ diag: 41:16 error "requested alignment is less than minimum alignment of 4 for type '(unnamed struct at "
//@ diag: 42:19 error "requested alignment is less than minimum alignment of 4 for type 'S6'"
//@ diag: 43:6 error "'alignas' attribute cannot be applied to an enumeration"
//@ diag: 44:12 error "'alignas' attribute cannot be applied to an enumeration"
//@ diag: 45:22 error "an attribute list cannot appear here"
//@ diag: 45:40 error "'alignas' attribute cannot be applied to a bit-field"
//@ diag: 45:66 error "an attribute list cannot appear here"
//@ diag: 46:1 error "requested alignment is less than minimum alignment of 4 for type 'int'"
//@ diag: 49:8 error "requested alignment is less than minimum alignment of 2 for type 'S10'"
//@ diag: 50:26 error "requested alignment is less than minimum alignment of 4 for type 'S11<int>'"
//@ diag: 51:10 note "in instantiation of template class 'S11<int>' requested here"
//@ diag: 53:8 error "requested alignment is less than minimum alignment of 4 for type 'S12'"
//@ diag: 54:7 error "requested alignment is less than minimum alignment of 4 for type 'U13'"
//@ diag: 55:14 error "requested alignment is less than minimum alignment of 4 for type 'int'"
//@ diag: 60:24 error "requested alignment is less than minimum alignment of 4 for type 'S19<1>'"
//@ diag: 61:8 note "in instantiation of template class 'S19<1>' requested here"
//@ diag: 64:12 error "requested alignment is less than minimum alignment of 4 for type 'int'"
//@ diag: 66:8 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 65:8 note "declared with 'alignas' attribute here"
//@ diag: 65:8 error "requested alignment is less than minimum alignment of 4 for type 'S20'"
//@ diag: 67:1 error "requested alignment is less than minimum alignment of 4 for type 'int'"
struct alignas(1) S1 { int i; };
struct alignas(2) S2 { int i; };
struct alignas(8) S3 { int i; };
struct alignas(1) S4 { char c; };
typedef struct alignas(1) { int i; } T5;
using T6 = struct alignas(1) S6 { int i; };
enum alignas(4) E7 { e7 };
enum class alignas(1) E8 : int { e8 };
struct B { int x : 3 alignas(4); int y alignas(4) : 3; int z : 3 [[deprecated]]; };
alignas(1) int v1;
struct alignas(1) alignas(8) S9 { int i; };
struct alignas(1) S10;
struct alignas(1) S10 { short s; };
template<class T> struct alignas(1) S11 { T t; };
S11<int> s11;
S11<char> s11c;
struct alignas(alignof(char)) S12 { int l; };
union alignas(2) U13 { int i; };
struct S14 { alignas(1) int m; alignas(1) alignas(4) int n; };
struct alignas(S1) S15 { short d; };
struct alignas(1) __attribute__((aligned(8))) S16 { int i; };
struct alignas(1) __attribute__((aligned)) S17 { int i; };
struct __attribute__((aligned(1))) S18 { int i; };
template<int N> struct alignas(N) S19 { int i; };
S19<1> s19;
S19<8> s19b;
alignas(0) int z1;
alignas(1) alignas(0) int z2;
struct alignas(1) S20;
struct S20 { int i; };
alignas(char) int v21;
