// __builtin_choose_expr ([clang] "__builtin_choose_expr"; clang 19
// ActOnChooseExpr): a constant condition selects one operand, which is
// the result unconverted (its type and value category, an lvalue, a
// class or a complex value); only the chosen operand is evaluated, in
// constant evaluation and at run time; a dependent condition is
// resolved on substitution (WP-307).
//@ expect: run
struct S { int a = 5; };
int x = 3;
static_assert(__builtin_choose_expr(1, 4, 2.0) == 4);
static_assert(__is_same(decltype(__builtin_choose_expr(0, 4, 2.0)), double));
static_assert(__is_same(decltype(__builtin_choose_expr(1, x, 2.0)), int&));
static_assert(__is_same(decltype((__builtin_choose_expr(1, x, 2.0))), int&));
template<class T> constexpr auto pick(T t) { return __builtin_choose_expr(sizeof(T) == 4, t + 1, 'c'); }
static_assert(pick(1) == 2);
static_assert(__is_same(decltype(pick(1.0)), char));
template<int N> int sel() { return __builtin_choose_expr(N, x, S{}.a); }
int main() {
  __builtin_choose_expr(1, x, 0) = 7;           // lvalue
  S s = __builtin_choose_expr(0, 1, S{});        // aggregate
  _Complex double z = __builtin_choose_expr(1, 1.0 + 2.0i, 0);
  int r = 0;
  if (x != 7) r |= 1;
  if (s.a != 5) r |= 2;
  if (__real__ z != 1.0) r |= 4;
  if (sel<1>() != 7 || sel<0>() != 5) r |= 8;
  __builtin_choose_expr(0, x = 1, x = 2);
  if (x != 2) r |= 16;
  return r;
}
