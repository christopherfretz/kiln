//@ expect: compile-fail
// [temp.deduct.type]/2: a pack deduced twice must deduce the same elements.
template <class... Ts> struct tl {};
template <class... Ts> void same(tl<Ts...>, tl<Ts...>);
void f() { same(tl<int>{}, tl<char>{}); }
