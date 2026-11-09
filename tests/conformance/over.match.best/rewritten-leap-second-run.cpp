// [over.match.best.general]/2.8, reduced from libstdc++'s
// `std::chrono::leap_second` (`__get_leap_second_info` compares
// `sys_seconds < leap_second` and `sys_seconds + 1s >= leap_second`; it
// is how fmt's chrono test differed from clang). For `t < ls` the
// candidates are the non-rewritten `operator<(const sys_time<D>&, const
// leap_second&)` and the reversed `operator<=>(const leap_second&, const
// sys_time<D>&)`, whose template parameter is constrained. They are
// equally specialized, and constraints are not compared because the
// declared parameter lists differ positionally (clang/GCC do not apply
// [temp.func.order]/6.2.1), so the non-rewritten candidate wins. Not
// reversed (`ls < t`), the constrained `<=>` is more specialized (/2.5).
//@ expect: run
//@ stdout: <(t,ls)|>=(t,ls)|==(ls,t)|<=>(ls,t)|<=>(ls,t)|
#include <compare>
extern "C" int printf(const char*, ...);
template <class T, class U> concept three_way_comparable_with = requires(T t, U u) { t <=> u; };
struct seconds {
  long v;
  friend constexpr auto operator<=>(seconds, seconds) = default;
};
template <class C, class D> struct time_point { D d; };
struct system_clock {};
template <class D> using sys_time = time_point<system_clock, D>;
class leap_second {
  seconds s{1};
public:
  constexpr seconds date() const { return s; }
  template <class D>
  friend bool operator==(const leap_second& x, const sys_time<D>& y) { printf("==(ls,t)|"); return x.date() == y.d; }
  template <class D>
  friend bool operator<(const sys_time<D>& x, const leap_second& y) { printf("<(t,ls)|"); return x.d < y.date(); }
  template <class D>
  friend bool operator>=(const sys_time<D>& x, const leap_second& y) { printf(">=(t,ls)|"); return !(x.d < y.date()); }
  template <three_way_comparable_with<seconds> D>
  friend auto operator<=>(const leap_second& x, const sys_time<D>& y) { printf("<=>(ls,t)|"); return x.date() <=> y.d; }
};
int main() {
  leap_second ls;
  sys_time<seconds> t{{0}};
  (void)(t < ls);
  (void)(t >= ls);
  (void)(t == ls);
  (void)(ls < t);
  (void)(ls >= t);
  printf("\n");
}
