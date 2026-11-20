// [cpp.pragma]/1: a '#pragma pack(push, ...)' never popped is reported at the
// end of the file (-Wpragma-pack); when '#pragma pack()' reset the value, a note
// suggests 'pop' instead. WP-1744.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 9:9 warning "unterminated '#pragma pack (push, ...)' at end of file"
//@ diag: 10:9 note "did you intend to use '#pragma pack (pop)' instead of '#pragma pack()'?"
#pragma pack(2)
#pragma pack(push, 1)
#pragma pack()
int x;
struct S { char c; int i; };
static_assert(sizeof(S) == 8);
