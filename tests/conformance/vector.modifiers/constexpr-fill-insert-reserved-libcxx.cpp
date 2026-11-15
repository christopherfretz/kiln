// [vector.modifiers], [class.union.general]/2: the fill-insert test of
// constexpr-fill-insert-reserved.cpp against libc++ (WP-1490).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ args: -stdlib=libc++
#include "constexpr-fill-insert-reserved.cpp"
