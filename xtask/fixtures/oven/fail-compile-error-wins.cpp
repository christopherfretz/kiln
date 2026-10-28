// The kind's own checks come first: a compile-ok test that fails to compile
// fails for that reason, whatever the budget says.
//@ expect: compile-ok
//@ oven: steps<=1000
static_assert(false);
