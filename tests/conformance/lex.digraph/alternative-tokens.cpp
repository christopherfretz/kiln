// [lex.digraph]: alternative tokens behave exactly like their primary
// spellings, including %: and %:%: in preprocessing directives.
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
%:define CAT(a, b) a %:%: b

struct S <% int v<:2:>; %>;

constexpr int f(int x, int y) <% return x and y ? x bitor 2 : compl 0; %>

static_assert(f(1, 1) == 3);
static_assert(f(0, 1) == -1);
static_assert(CAT(1, 0) == 10);
static_assert(sizeof(S) == 2 * sizeof(int));
