// [temp.variadic]/5, [temp.deduct.call]/1, [temp.inst]/3: a member
// function template `run(Ts&&... ts, Us&&... us)` of a class template
// with pack `Ts` has two function parameter packs. Instantiating the class
// expands `ts` to one parameter per element of `Ts`; `Us` is deduced from
// the remaining arguments. In the body of the specialization, `ts` names
// the parameters from `Ts` and `us` those from `Us`, whatever their
// lengths, so each expansion pairs the elements of its own pack.
// libstdc++'s `__tuple_concater::_S_do(_Tp&&, _Tpls&&... __tps,
// _Us&&... __us)` (tuple:2129), reached from `std::tuple_cat`.
// Verified against clang 21.
//@ expect: run
//@ strict-diags
//@ stdout: 37 12 3 7 0 54
#include <stdio.h>

template <class T> T &&fwd(T &x) { return static_cast<T &&>(x); }
int sum() { return 0; }
template <class H, class... R> int sum(H h, R... r) { return h + sum(r...); }

template <class... Ts> struct C {
  template <class... Us> static int run(Ts &&...ts, Us &&...us) {
    return sum(fwd<Ts>(ts)...) * 10 + sum(fwd<Us>(us)...);
  }
  // The packs' sizes and their parameters named without `fwd`.
  template <class... Us> static int sizes(Ts &&...ts, Us &&...us) {
    return (int)sizeof...(ts) * 10 + (int)sizeof...(us) + 0 * sum(ts..., us...);
  }
  // A non-pack parameter before both packs, by value.
  template <class... Us> static int mixed(int a, Ts... ts, Us... us) {
    return a * 10 + sum(us...) - sum(ts...);
  }
};

struct A { operator int() const { return 1; } };
struct B { operator int() const { return 2; } };

int main() {
  printf("%d %d %d %d %d %d\n", C<A, B>::run(A{}, B{}, 3, 4), C<A>::sizes(A{}, 1, 2),
         C<>::run(1, 2), C<A, B>::run(A{}, B{}, 7) - 30, C<A, B>::run(A{}, B{}) - 30,
         C<B>::mixed(5, B{}, 1, 2, 3));
}
