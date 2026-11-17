// [dcl.init.list]/3.9, /7: a narrowing conversion in a list-initialization
// is ill-formed, also in a system header. kiln keeps clang 22's behaviour
// (a recorded decision, WP-1538, docs/status/deviations.md "Narrowing in
// a system header"): clang's narrowing error is a warning that defaults to
// an error (`-Wc++11-narrowing`), and like every warning it is not shown in
// a system header without `-Wsystem-headers`, so a narrowing in a header
// found through `-isystem` (a constant one, a non-constant one, one
// instantiated from a user file) gets no diagnostic. GCC 16.1 rejects the
// constant one (`int i = {2.4};`) and is silent on the others. Found
// running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
//@ args: -isystem %{test_dir}/Inputs
#include <narrowing_system_header.h>
int main() { return sys_narrow(1.0) + sys_tnarrow(2.0) + sys_mk(3).c - 6 + sys_narrowed - 2; }
