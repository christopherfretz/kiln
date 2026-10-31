// [temp.deduct.call]/3: a forwarding-reference pattern deduces lvalue references.
template <class... Ts> constexpr bool all_lvalue_refs(Ts &&...) { return (__is_lvalue_reference(Ts) && ...); }
int a, b;
static_assert(all_lvalue_refs(a, b));
static_assert(!all_lvalue_refs(a, 1));
static_assert(all_lvalue_refs());
