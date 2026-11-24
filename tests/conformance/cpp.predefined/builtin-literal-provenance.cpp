//@ expect: ast
//@ ast-filter: lits
//@ args: -fkiln-builtin-literals
//@ skip(clang): -fkiln-builtin-literals is kiln's (spec/ast.md §22)
// [cpp.predefined]/1, [cpp.stringize], [cpp.concat], spec/ast.md §22: with
// the builtin-macro literal table on, the dump names the builtin macro a
// literal came from, directly, through a user macro, as a macro argument,
// stringized and pasted; a written literal, and a stringized macro name,
// name none.
#define LN __LINE__
#define ID(x) x
#define STR2(x) #x
#define STR(x) STR2(x)
#define CAT2(a, b) a##b
#define CAT(a, b) CAT2(a, b)
namespace lits {
int line = __LINE__;
int line_user = LN;
int line_arg = ID(__LINE__);
int counter = __COUNTER__;
int level = __INCLUDE_LEVEL__;
const char *name = __FILE_NAME__;
const char *name_concat = "<" __FILE_NAME__ ">";
const char *line_string = STR(__LINE__);
int line_pasted = CAT(1, __LINE__);
int written = 42;
const char *macro_name = STR2(__LINE__);
}
