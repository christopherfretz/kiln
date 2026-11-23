// [dcl.enum]/3, [dcl.align], GNU `aligned` on a redeclaration of an
// opaque enumeration with no definition: clang 19 lays the enumeration
// out from its first declaration (`TagType::getDecl`), so an `aligned`
// added by a later declaration does not change it, whenever it is asked;
// one on the first declaration is kept. clang 22 reads the declaration
// the use names instead (the sibling without -19). kiln folds an
// enumeration's redeclarations into its first declaration and so applies
// the later `aligned` to it.
//@ requires: clang<22
//@ xfail(kiln): WP-2840 follow-up, kiln folds an enum redeclaration's aligned into its first declaration
enum O1 : int;
enum __attribute__((aligned(8))) O1 : int;
static_assert(alignof(O1) == 4);
struct S1 { char c; O1 e; };
static_assert(sizeof(S1) == 8);

enum __attribute__((aligned(8))) O2 : int;
enum O2 : int;
static_assert(alignof(O2) == 8);
