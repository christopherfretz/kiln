// __builtin_addressof (spec/sema-templates.md §9.5, [specialized.addressof]):
// the address of an lvalue, also in constant expressions and of
// functions.
//@ expect: run
//@ strict-diags
struct S { int x; };
S s;
constexpr int g = 3;
static_assert(__builtin_addressof(g) == &g);
static_assert(*__builtin_addressof(g) == 3);
S *ps = __builtin_addressof(s);
constexpr S cs{4};
static_assert(__builtin_addressof(cs.x) == &cs.x);
static_assert(__builtin_addressof(cs)->x == 4);
constexpr int f() {
  int a = 5;
  int *p = __builtin_addressof(a);
  *p = 6;
  return a;
}
static_assert(f() == 6);
static_assert(__is_same(decltype(__builtin_addressof(cs)), const S *));
void h() {}
void (*ph)() = __builtin_addressof(h);
int main() {
  int y = 1;
  *__builtin_addressof(y) = 0;
  if (ph != &h)
    return 2;
  return y + (ps == &s ? 0 : 1);
}
