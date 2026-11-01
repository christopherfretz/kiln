// [over.ics.rank]/3.1.2 (P0388): of two list-initialization sequences
// converting to arrays of the same element type, the one initializing
// fewer elements is better, and with as many elements, the one that does
// not convert to an array of unknown bound is better, even if another
// rule of /3 would apply.
constexpr int f(const int (&)[3]) { return 3; }
constexpr int f(const int (&)[]) { return 0; }
static_assert(f({1, 2, 3}) == 3);

constexpr int g(const int (&)[2]) { return 2; }
constexpr int g(const int (&)[4]) { return 4; }
static_assert(g({1}) == 2);
static_assert(g({1, 2}) == 2);
static_assert(g({1, 2, 3}) == 4);

// The element types are compared without their cv-qualifiers.
constexpr int h(int (&&)[2]) { return 2; }
constexpr int h(const int (&)[3]) { return 3; }
static_assert(h({1}) == 2);

// Different element types: the element conversions decide.
constexpr int m(const int (&)[2]) { return 2; }
constexpr int m(const long (&)[1]) { return 1; }
static_assert(m({1}) == 2);

// Elements initialized by aggregate initialization (a user-defined
// conversion sequence per element).
struct S { int x; };
constexpr int n(const S (&)[3]) { return 3; }
constexpr int n(const S (&)[2]) { return 2; }
static_assert(n({{1}, {2}}) == 2);

// Arrays of arrays: the outer bound decides.
constexpr int p(int (&&)[2][2]) { return 22; }
constexpr int p(int (&&)[3][2]) { return 32; }
static_assert(p({{1, 2}}) == 22);
