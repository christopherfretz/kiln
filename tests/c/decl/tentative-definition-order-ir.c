// An object defined only by tentative definitions (C23 6.9.2p2) is defined
// at the end of the translation unit (clang Sema `TentativeDefinitions`,
// `CompleteTentativeDefinition`): after the definitions with initializers,
// in the order of each object's first tentative definition. A reference
// from a function emitted earlier has already placed the global in the
// module, so it keeps that position; an unused `static` one is not emitted.
// spec/codegen.md "C tentative definitions".
//@ expect: ir
//@ args: -std=gnu17 -Wno-tentative-definition-array
int a;
int b = 1;
static int c;
int d;
int e;
int d;
int e = 5;
static int f2;
int arr[];
int z;
static int unused;
int uses(void) { return c + f2 + z; }
int a;
int late = 3;
static int sl = 2;
int usesl(void) { return sl; }
int q;
struct T t;
struct T { int x, y; };
