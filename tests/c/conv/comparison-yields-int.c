// C17 6.5.8p6, 6.5.9p3, 6.5.13p3, 6.5.14p3, 6.5.3.3p5: relational,
// equality, logical operators and ! yield int, not bool.
//@ expect: compile-ok
//@ strict-diags
int x, y;
_Static_assert(_Generic(x < y, int: 1, default: 0), "<");
_Static_assert(_Generic(x == y, int: 1, default: 0), "==");
_Static_assert(_Generic(x != y, int: 1, default: 0), "!=");
_Static_assert(_Generic(x && y, int: 1, default: 0), "&&");
_Static_assert(_Generic(x || y, int: 1, default: 0), "||");
_Static_assert(_Generic(!x, int: 1, default: 0), "!");
_Static_assert(sizeof(x < y) == sizeof(int), "");
_Static_assert(_Generic((_Bool)1, _Bool: 1, default: 0), "_Bool cast");
_Static_assert(_Generic((_Bool)1 + (_Bool)1, int: 1, default: 0), "_Bool promotes");
