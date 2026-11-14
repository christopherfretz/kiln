// [temp.arg.general]/1, [temp.expl.spec]/1: the partial specialization
// `promote<T, false>` that a non-integer default selects has no `type`
// member; through the snapshot the default still selects it
// (WP-1320).
//@ expect: compile-fail
//@ diag: 10:27 error "no type named 'type' in 'nds::promote<S>'"
#include "Inputs/nttp_default_spec.h"

struct S {};
typename nds::promote<S>::type x;
