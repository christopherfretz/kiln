// [cpp.cond]/4-/6: `__has_include` found, with the file and search-path
// index, and not found; a name from a macro, written and glued from
// `<...>`; `__has_cpp_attribute` with its value; feature queries in
// ordinary text, an unknown operand evaluating to 0 among them. Each
// query is a row of its builtin's expansion (spec/pp.md "Preprocessing
// record", queries).
//@ expect: compile-ok
//@ strict-diags
//@ args: -I%{test_dir}/Inputs
//@ pp-record: has-include.record
#if __has_include(<present.h>)
int found;
#endif
#if __has_include("nope.h")
#error found
#endif
#define HDR "present.h"
#if __has_include(HDR)
int from_macro;
#endif
#define ANGLED <present.h>
#if __has_include(ANGLED)
int glued;
#endif
#if __has_cpp_attribute(nodiscard) >= 201907L
int attr;
#endif
int feature = __has_feature(cxx_rvalue_references);
int unknown = __has_cpp_attribute(no_such_attribute);
int scoped = __has_cpp_attribute(gnu::unused);
int warning = __has_warning("-Wall");
bool ident = __is_identifier(foo);
int builtin = __has_builtin(__builtin_expect);
