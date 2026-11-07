// [dcl.contract.func], [dcl.fct.def.coroutine]/7, [basic.contract.eval]
// (P2900R14 §3.5.2): coroutine postconditions under ignore. Each runs
// once, when the ramp returns to the caller, on the return object (after
// get_return_object() and the initial suspend, and for an eager coroutine
// after its first suspension; never on a resume); no predicate is
// evaluated.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=ignore
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: gro
//@ stdout: initial
//@ stdout: returned
//@ stdout: body
//@ stdout: 6
//@ stdout: gro
//@ stdout: initial
//@ stdout: 5
//@ stdout: eager 1
//@ stdout: returned eager
//@ stdout: eager 2
//@ stdout: after bad
#include "Inputs/coroutine-post.h"
