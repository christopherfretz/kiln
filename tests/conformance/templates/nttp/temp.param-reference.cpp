// [temp.arg.nontype]: reference non-type template parameters bind to
// objects and functions with static storage duration; the parameter names
// the referent (an lvalue).
//@ expect: compile-ok
struct A { int x; };
constexpr A ga{5};
int gi = 3;
const int ci = 7;
template <const A& a> constexpr int ax() { return a.x; }
static_assert(ax<ga>() == 5);
template <const A& a> constexpr const A* pa() { return &a; }
static_assert(pa<ga>() == &ga);
template <int& r> constexpr int& get() { return r; }
template <const int& r> constexpr int cv() { return r; }
static_assert(cv<ci>() == 7);
static_assert(&get<gi>() == &gi);
static_assert(__is_same(decltype(get<gi>()), int&));
void foo();
template <void (&f)()> constexpr bool same() { return &f == &foo; }
static_assert(same<foo>());
