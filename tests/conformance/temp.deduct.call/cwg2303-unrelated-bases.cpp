// [temp.deduct.call]/5: two matching bases neither of which derives from
// the other leave more than one possible deduced A, so deduction fails
// (overload resolution may then pick another candidate).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:12 error "no matching function for call to 'g'"
//@ diag: 14:28 note "candidate template ignored: failed template argument deduction"
template <class... T> struct A {};
template <class... T> constexpr int f(const A<T...> &) { return 1; }
constexpr int f(...) { return 2; }
struct U : A<int>, A<char> {};
static_assert(f(U{}) == 2);

template <class... T> void g(const A<T...> &);
void t() { g(U{}); }
