// The written-reference table (spec/ast.md §18.4, stage 3), role
// `designator`: the member a field designator names
// ([dcl.init.aggr]/3.1), found in the aggregate's class when the
// initialization is checked, with `how` member (the `MEMBER` flag). A
// nested designator (`.inner.depth`, the C99 extension) has one entry
// per member. A member of an anonymous union is the member itself, not
// the anonymous member that contains it ([class.union.anon]/1). In a
// template pattern the aggregate of a dependent type is not known: the
// entry is `dep`. In overload resolution the chosen candidate's
// aggregate decides. An offsetof member-designator
// ([support.types.layout]/1, `__builtin_offsetof`) names its members
// the same way: one entry per member, the anonymous union's member
// itself, `dep` for a dependent operand type.
//@ written-refs: designators.refs
//@ args: -Wno-c99-designator
struct Inner {
  int depth;
};
struct Config {
  int width;
  Inner inner;
  union {
    int as_int;
    float as_float;
  };
};
struct Other {
  long width;
};

Config a = {.width = 1, .inner = {.depth = 2}};
Config b = {.inner.depth = 3, .as_float = 1.0f};
Config c{.as_int = 4};

int pick(Other);
int chosen = pick({.width = 5});

template <class T> T make() { return T{.width = 6}; }
template <class T> Config fixed() { return Config{.width = sizeof(T)}; }

static_assert(__builtin_offsetof(Config, inner.depth) == sizeof(int));
static_assert(__builtin_offsetof(Config, as_float) > 0);
template <class T> constexpr unsigned long at = __builtin_offsetof(T, width);
