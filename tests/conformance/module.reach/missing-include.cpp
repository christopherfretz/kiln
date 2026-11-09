// [module.global.frag]/3, [module.reach]/3: a use of a global module
// fragment declaration of an imported unit is reported as clang 22 does,
// naming the header to include when one is known: walking out from the
// declaration's file along its #includes, the first include-guarded file
// (a controlling macro; `#pragma once` does not count), named relative to
// the longest search directory holding it (`<...>` for a system one) or
// else to the main file's directory (`"..."`). An unguarded header, or
// one only under `#pragma once`, gets the plain "missing '#include'".
//@ expect: compile-fail
//@ std: c++20
//@ args: -isystem %{test_dir}/Inputs/sys
//@ module: mr.h=Inputs/mr-h.cppm
//@ strict-diags
//@ diag: 20:9 error "missing '#include \"Inputs/mr-g.h\"'; 'guarded_fn' must be declared before it is used"
//@ diag: 21:9 error "missing '#include \"Inputs/mr-g.h\"'; 'nested_fn' must be declared before it is used"
//@ diag: 22:9 error "missing '#include'; 'once_fn' must be declared before it is used"
//@ diag: 23:9 error "missing '#include'; 'unguarded_fn' must be declared before it is used"
//@ diag: 24:9 error "missing '#include <mr-s.h>'; 'sys_fn' must be declared before it is used"
import mr.h;
int a = guarded_fn();
int b = nested_fn();
int c = once_fn();
int d = unguarded_fn();
int e = sys_fn();
int f = h_fn();
