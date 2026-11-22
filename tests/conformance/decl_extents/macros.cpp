// The declaration-extent table (spec/ast.md §19.6): the bounds are raw
// token locations, so a declaration from a macro body or with a name
// from a macro argument has macro locations; the dump prints spelling
// positions and the expansion point of a full range that starts in a
// macro (`exp=`). Nothing is mapped to file ranges.
//@ decl-extents: macros.extents
#define DECL(type, name) type name = 0;
#define NAME(n) n
#define SEMI ;
#define BEGIN_NS(n) namespace n {
#define END_NS }
#define GLUE(a, b) a##b
DECL(int, from_macro)
int NAME(named_by_arg) SEMI
BEGIN_NS(wrapped)
int inside;
END_NS
int GLUE(pas, ted);
