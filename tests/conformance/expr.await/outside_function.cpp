// [expr.await]/2: an await-expression shall appear only in a
// potentially-evaluated expression within the compound-statement of a
// function-body; not in a namespace-scope initializer.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:9 error "'co_await' cannot be used outside a function"
#include "../dcl.fct.def.coroutine/Inputs/coro.h"
int x = co_await std::suspend_never{};
