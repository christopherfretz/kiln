// [dcl.init.aggr]/5, [dcl.init.list]/3.4, [temp.inst]: a braced
// initializer of an aggregate whose array member the list leaves out
// (`S addr{};` with `char path[108]`, reduced from real code's
// `sockaddr_un addr{};` in a lambda in a function template), in a templated
// entity at -std=c++17: each specialization initializes the variable again
// from the written list, so the omitted array member is value-initialized
// rather than initialized from an expression of the array's own type
// (WP-613). Covers `{}`, `= {}`, a partial list, `static`, a dependent
// type, nested aggregates, a member holding the array, a default member
// initializer, arrays of the aggregate, assignment from a list, a lambda in
// a function template and a member function of a class template.
//@ expect: run
//@ std: c++17
#include "Inputs/array-member-filler-in-template.h"

int main() { return run_all(); }
