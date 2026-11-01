// [temp.mem]/1, [class.mem]/7: the member function bodies, default arguments and default member
// initializers of a member class template of a non-template class are complete-class contexts
// parsed after the enclosing class; the member template's parameters are still in scope there
// (libc++ <variant>: `__visitation::__base::__dispatcher<_Is...>::__dispatch`)
//@ expect: run
//@ strict-diags
struct acc {
  template <int I, class T> static constexpr int get(T t) { return I + t; }
};
struct vis {
  struct base {
    template <int... Is>
    struct disp {
      template <class F, class... Vs>
      static constexpr int dispatch(F f, Vs... vs) { return f(acc::get<Is>(vs)...); }
    };
  };
  template <int J>
  struct one {
    int m = J;
    static constexpr int get(int v = J) { return J + v; }
    int self() const { return m + sizeof(one<J>); }
  };
};
constexpr int add(int a, int b) { return a + b; }
static_assert(vis::base::disp<1, 2>::dispatch(add, 10, 20) == 33);
static_assert(vis::one<1>::get() == 2 && vis::one<1>::get(10) == 11);
int main() { return vis::one<3>().self() == 3 + sizeof(int) ? 0 : 1; }
