// [temp.deduct.call]/1, [dcl.type.auto.deduct]/2.1: with only nested
// braced-init-list elements (non-deduced) or a designated element, nothing
// deduces `U` in `std::initializer_list<U>`.
//@ verify
#include <initializer_list>
struct A { int x; };
auto a = {{1}};     // expected-error {{cannot deduce actual type for variable 'a' with type 'auto' from initializer list}}
auto b = {.x = 1};  // expected-error {{cannot deduce actual type for variable 'b' with type 'auto' from initializer list}}
