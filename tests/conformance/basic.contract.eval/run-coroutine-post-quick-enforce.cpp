// [dcl.contract.func], [dcl.fct.def.coroutine]/7, [basic.contract.eval]
// (P2900R14 §3.5.2): coroutine postconditions under quick_enforce. Each
// runs once, when the ramp returns to the caller, on the return object
// (after get_return_object() and the initial suspend, and for an eager
// coroutine after its first suspension; never on a resume); the violation
// of `bad` terminates without calling the handler.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=quick_enforce
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ exit: 7
//@ stdout: pre
//@ stdout: gro
//@ stdout: initial
//@ stdout: post
//@ stdout: returned
//@ stdout: body
//@ stdout: 6
//@ stdout: gro
//@ stdout: initial
//@ stdout: post conv
//@ stdout: 5
//@ stdout: eager 1
//@ stdout: post eager
//@ stdout: returned eager
//@ stdout: eager 2
//@ stdout: post bad
//@ stdout: terminate
#include "Inputs/coroutine-post.h"
