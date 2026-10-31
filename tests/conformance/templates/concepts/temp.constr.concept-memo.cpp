//@ expect: compile-ok
// Satisfaction is memoized per (concept, arguments): repeated checks agree.
template <class T> concept Big = sizeof(T) > 4;
static_assert(Big<double> && Big<double> && Big<double>);
static_assert(!Big<int> && !Big<int>);
template <Big T> constexpr int f(T) { return 1; }
static_assert(f(1.0) + f(2.0) + f(3.0) == 3);
