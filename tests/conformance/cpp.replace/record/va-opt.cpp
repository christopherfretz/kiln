// [cpp.subst]/3 (`__VA_OPT__`), [cpp.invoke]: an omitted variadic argument
// is flagged and has no span; an empty one is an empty span at the `)`;
// `x, y` is one span; the `,` from `__VA_OPT__` is a body token (spec/pp.md
// "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
//@ pp-record: va-opt.record
int f(int, ...);
int x, y;
#define F(a, ...) f(a __VA_OPT__(,) __VA_ARGS__)
int r1 = F(1);
int r2 = F(1,);
int r3 = F(1, x, y);
