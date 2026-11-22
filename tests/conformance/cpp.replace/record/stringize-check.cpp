// [cpp.stringize], [cpp.subst]: the preprocessing record (spec/pp.md
// "Preprocessing record") of a checking macro reduced from real code: one
// expansion whose argument 0 spans `L::ok(x)` in the file; `L` comes from
// argument 0 at its file location; parameter `c` has one plain use and
// one `#` use, and the string is stringized from argument 0.
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: stringize-check.record
namespace L { bool ok(int); }
int fail(const char *);
int x;
#define CHECK(c) ((c) ? 0 : fail(#c))
int f() { return CHECK(L::ok(x)); }
