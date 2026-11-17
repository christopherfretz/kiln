// C17 6.7.2.2, GNU `aligned` on an enumeration (before its tag, after its
// closing brace, or on a typedef of it): it raises the alignment
// `_Alignof` reports and the layout of a structure holding it uses, not
// its size, as clang 22 lays it out; an array of it whose element size is
// not a multiple of that alignment is rejected (clang
// `err_array_element_alignment`). GCC 16.1 ignores the attribute on the
// enumeration type in C (alignment 4, `struct H` 8 bytes): kiln follows
// clang, whose layout it shares, so C code mixing the two compilers on
// such a type already disagrees between clang and GCC. Found running
// GCC's C++ testsuite.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:11 error "size of array element of type 'enum E' (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 28:6 error "size of array element of type 'TF' (aka 'enum F') (4 bytes) isn't a multiple of its alignment (8 bytes)"
enum __attribute__((aligned(16))) E { e };
enum F { f } __attribute__((aligned(8)));
typedef enum F TF;
_Static_assert(sizeof(enum E) == 4 && _Alignof(enum E) == 16, "");
_Static_assert(sizeof(TF) == 4 && _Alignof(TF) == 8, "");
struct H { char c; enum E x; };
_Static_assert(__builtin_offsetof(struct H, x) == 16 && sizeof(struct H) == 32, "");
enum __attribute__((aligned(4))) Q { q };
enum Q qa[2];
_Static_assert(sizeof(qa) == 8, "");
int main(void) { return 0; }
enum E bad[2];
TF bf[2];
