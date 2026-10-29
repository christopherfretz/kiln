// [over.ics.rank]/3.2, [over.ics.scs]: exact match beats promotion beats
// conversion; qualification adjustments and the reference-binding
// tie-breakers rank standard conversion sequences.
//@ expect: compile-ok
template <class T, class U> constexpr bool same = __is_same(T, U);
struct T1 {}; struct T2 {}; struct T3 {}; struct T4 {};

T1 rank(int);
T2 rank(double);
static_assert(same<decltype(rank('a')), T1>);  // promotion over conversion
static_assert(same<decltype(rank(1.0f)), T2>); // promotion over conversion

T1 q(const int *);
T2 q(int *);
int i;
const int ci = 0;
static_assert(same<decltype(q(&i)), T2>);  // identity over qualification
static_assert(same<decltype(q(&ci)), T1>);

T1 r(int &);
T2 r(const int &);
static_assert(same<decltype(r(i)), T1>);   // less cv-qualified referee
static_assert(same<decltype(r(1)), T2>);

T1 rv(int &&);
T2 rv(const int &);
static_assert(same<decltype(rv(1)), T1>);  // rvalue binds rvalue reference
static_assert(same<decltype(rv(i)), T2>);

void fn();
T1 fr(void (&)());
T2 fr(void (&&)());
static_assert(same<decltype(fr(fn)), T1>); // function lvalue: lvalue reference

T1 b(bool);
T2 b(void *);
static_assert(same<decltype(b(&i)), T2>);  // pointer to void* over bool

struct Base {}; struct Mid : Base {}; struct Leaf : Mid {};
T1 d(Base *);
T2 d(Mid *);
Leaf leaf;
static_assert(same<decltype(d(&leaf)), T2>); // nearer base
T3 dr(Base &);
T4 dr(Mid &);
static_assert(same<decltype(dr(leaf)), T4>);

T1 e(int);
T2 e(long);
enum E : short { e0 };
enum class S { s0 };
static_assert(same<decltype(e(e0)), T1>);  // promotion of the underlying type
T1 s(S);
T2 s(int);
static_assert(same<decltype(s(S::s0)), T1>);
