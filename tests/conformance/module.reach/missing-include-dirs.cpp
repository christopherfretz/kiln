// [module.global.frag]/3, [module.reach]/3: the header named in "missing
// '#include'" for an imported unit's global module fragment declaration,
// as clang 22 and 19 name it. The header's path is absolute (clang reads
// it back from the BMI) and is compared with the search directories as
// written, so a relative one (`-I tests/...`) never matches and the
// header is named relative to the main file's directory instead; an
// absolute one is stripped. The walk out along the #includes passes
// through an `#include` written with a macro: the unguarded mr-i.h is
// reached from the guarded mr-m.h.
//@ expect: compile-fail
//@ std: c++20
//@ args: -I tests/conformance/module.reach/Inputs/rel -I %{test_dir}/Inputs/abs
//@ module: mr.d=Inputs/mr-d.cppm
//@ strict-diags
//@ diag: 19:9 error "missing '#include \"Inputs/rel/mr-r.h\"'; 'rel_fn' must be declared before it is used"
//@ diag: 20:9 error "missing '#include \"mr-a.h\"'; 'abs_fn' must be declared before it is used"
//@ diag: 21:9 error "missing '#include \"mr-m.h\"'; 'inner_fn' must be declared before it is used"
import mr.d;
int a = rel_fn();
int b = abs_fn();
int c = inner_fn();
int d = d_fn();
