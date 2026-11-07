// [expr.prim.req.compound]/1.3, [temp.variadic]/5: the type-constraint
// of a return-type-requirement may expand a pack of the enclosing
// requires-expression's template (`-> awaiter<P...>`, as a sender
// library's awaitable concept writes it); the constraint checked is
// `awaiter<decltype((E)), P1, ..., Pn>` (WP-789).
//@ expect: compile-ok
//@ std: c++20
template <class A, class... P>
concept awaiter = sizeof...(P) < 2 && sizeof(A) > 0;

template <class A, class... P> int get(A, P &...) { return 0; }

template <class A, class... P>
concept awaitable = requires(A &&a, P &...p) {
  { get(a, p...) } -> awaiter<P...>;
};

static_assert(awaitable<int>);
static_assert(awaitable<int, long>);
static_assert(!awaitable<int, long, char>);

template <class... P>
concept all_small = requires(P... p) {
  { (sizeof(p) + ... + 0) } -> awaiter<P..., P...>;
};
static_assert(all_small<>);
static_assert(!all_small<int>);
