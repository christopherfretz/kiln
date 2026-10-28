// [lex.digraph]/2, [lex.key]: alternative tokens are operators wherever they
// appear, including in macro definitions and arguments; they are still
// identifiers to the preprocessor's # operator.
//@ expect: compile-ok
//@ strict-diags
#define AND and
#define BOTH(a, b) ((a) AND (b))
#define APPLY(op, a, b) ((a) op (b))
#define STR(x) #x
#define XSTR(x) STR(x)
static_assert(BOTH(true, true));
static_assert(not BOTH(true, false));
static_assert(APPLY(bitor, 4, 1) == 5);
static_assert(APPLY(xor, 6, 3) == 5);
static_assert(APPLY(not_eq, 1, 2));
static_assert(compl APPLY(bitand, 0, 1) == -1);
constexpr char s[] = STR(and);
static_assert(sizeof(s) == 4 and sizeof(s) == sizeof("and"));
constexpr char t[] = XSTR(AND);
static_assert(sizeof(t) == 4);
%:define HASH_DIGRAPH 1
static_assert(HASH_DIGRAPH == 1);
constexpr int arr<:2:> = <%1, 2%>;
static_assert(arr[1] == 2);
