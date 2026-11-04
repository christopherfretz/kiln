// [temp.res.general]/6, [over.oper.general], [over.match.oper]/2: resolved
// overloaded `->*` and `,` and the prefix/postfix `++`/`--` operator calls
// in a function template are formed again at instantiation by overload
// resolution on the substituted operands.
//@ expect: compile-ok
//@ strict-diags

struct S {
  int a = 1, b = 2;
};

struct MP {
  S *s;
  constexpr int operator->*(int S::*m) const { return s->*m; }
};

struct Seq {
  int v;
};
constexpr Seq operator,(Seq l, Seq r) { return Seq{l.v * 10 + r.v}; }

struct Ctr {
  int n = 0;
  constexpr Ctr &operator++() { ++n; return *this; }
  constexpr Ctr operator++(int) { Ctr c = *this; n += 10; return c; }
  constexpr Ctr &operator--() { --n; return *this; }
  constexpr Ctr operator--(int) { Ctr c = *this; n -= 100; return c; }
};

template <typename T> constexpr int ops(T t) {
  S s;
  MP mp{&s};
  int r = (mp->*&S::a) + (mp->*&S::b) * t; // 1 + 2t
  Seq q = (Seq{1}, Seq{2}, Seq{3});
  r = r * 1000 + q.v; // 123
  Ctr c;
  ++c;
  c++;
  --c;
  c--;
  return r * 1000 + c.n; // 1 + 10 - 1 - 100 = -90
}
static_assert(ops(2) == 5123000 - 90);
