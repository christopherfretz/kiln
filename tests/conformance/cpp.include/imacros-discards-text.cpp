// [cpp.include]: the text of an `-imacros` file never reaches the
// parser, the pragmas it holds included: `#pragma pack(1)` and
// `_Pragma("pack(2)")` there pack nothing, and `int clash;` declares
// nothing, while its `#define` is in force (clang
// HandleIncludeMacrosDirective discards every token up to its marker).
// WP-2822.
//@ expect: compile-ok
//@ strict-diags
//@ args: -imacros tests/conformance/cpp.include/Inputs/imacros/decls.h
struct S { char c; int i; };
static_assert(sizeof(S) == 2 * sizeof(int));
char clash;
static_assert(P == 1);
int main() { return clash; }
