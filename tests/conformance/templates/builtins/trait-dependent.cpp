// [temp.dep.constexpr]/4, [temp.inst]: a trait whose operand is dependent
// is value-dependent and is evaluated at instantiation
// (spec/sema-templates.md §9.1).
//@ strict-diags
struct S { virtual void f(); };
template <class T> struct Traits {
  static constexpr bool integral = __is_integral(T);
  static constexpr bool poly = __is_polymorphic(T);
  static constexpr bool same_int = __is_same(T, int);
  static constexpr unsigned long rank = __array_rank(T);
  static constexpr bool lv = __is_lvalue_expr(T());
};
static_assert(Traits<int>::integral && !Traits<double>::integral);
static_assert(Traits<S>::poly && !Traits<int>::poly);
static_assert(Traits<int>::same_int && !Traits<long>::same_int);
static_assert(Traits<int>::rank == 0 && !Traits<int>::lv);

template <class T, unsigned long N = __array_extent(T, 0)> struct Extent {
  static constexpr unsigned long value = N;
};
static_assert(Extent<int[7]>::value == 7 && Extent<int>::value == 0);

template <bool B> struct Bool { static constexpr bool value = B; };
template <class T> struct IsVoid : Bool<__is_void(T)> {};
static_assert(IsVoid<void>::value && !IsVoid<int>::value);
