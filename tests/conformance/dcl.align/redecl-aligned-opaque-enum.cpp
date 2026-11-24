// [dcl.enum]/3, [dcl.align], GNU `aligned` on a redeclaration of an
// opaque enumeration with no definition: clang 22 lays the enumeration
// out from the declaration the use names, which carries the strictest
// `aligned` of the declarations before it, so the answer depends on
// where it is asked (clang 19 reads the first declaration: the -19
// sibling).
//@ requires: clang>=22
//@ xfail(kiln): WP-2840 follow-up, kiln folds an enum redeclaration's aligned into its first declaration
enum O1 : int;
static_assert(alignof(O1) == 4);
struct S1 { char c; O1 e; };
enum __attribute__((aligned(8))) O1 : int;
static_assert(alignof(O1) == 8);
struct S2 { char c; O1 e; };
static_assert(sizeof(S1) == 8);
static_assert(sizeof(S2) == 16);
