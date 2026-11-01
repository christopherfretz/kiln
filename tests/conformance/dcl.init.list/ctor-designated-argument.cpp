// [dcl.init.list]/3.7, [over.match.list]/1.2: list-initialization of a
// non-aggregate class considers its constructors with the list's elements
// as arguments; an element that is itself a designated-initializer-list
// initializes an aggregate parameter ([dcl.init.aggr]/3.1). WP-179 (the
// designator case of the second pass; a designated list for the
// non-aggregate itself is not supported yet).
//@ expect: compile-ok

struct A { int x, y; };
struct S {
  int v;
  constexpr S(A a) : v(a.x * 10 + a.y) {}
  constexpr S(A a, int k) : v(a.x * 100 + a.y * k) {}
};
constexpr S s1{{.x = 1, .y = 2}};
static_assert(s1.v == 12);
constexpr S s2{{.y = 3}, 2};
static_assert(s2.v == 6);
constexpr S s3({.x = 4});
static_assert(s3.v == 40);
