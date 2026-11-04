// Shared body of array-member-filler-in-template*.cpp (WP-613): a braced
// initializer of an aggregate whose array member the list leaves out, in a
// templated entity. Each specialization initializes the variable again from
// the written list; the omitted array member is copy-initialized from an
// empty initializer list (value-initialized, [dcl.init.aggr]/5.2), not from
// an expression of the array's own type.
struct S {
  unsigned short fam;
  char path[108];
};
struct I {
  int a;
  int b[4];
};
struct O {
  S s;
};
struct Arr {
  char data[108];
};
struct H {
  int tag;
  Arr arr;
};
struct D {
  int a = 7;
  char p[3];
};

template <class T> bool zero(const T& x, unsigned short fam) {
  if (x.fam != fam) return false;
  for (char c : x.path)
    if (c != 0) return false;
  return true;
}
template <class T> bool zeros(const T* p, int n) {
  for (int i = 0; i < n; ++i)
    if (p[i] != 0) return false;
  return true;
}

template <class T> int braces(int n) {
  S a{};
  S b = {};
  S c{1};
  S d{1, {2, 3}};
  static S e{};
  if (!zero(a, 0) || !zero(b, 0) || !zero(c, 1) || !zero(e, 0)) return 1;
  if (d.fam != 1 || d.path[0] != 2 || d.path[1] != 3 || !zeros(d.path + 2, 106)) return 2;
  return n;
}

template <class T> int dependent(int n) {
  T a{};
  T b{5};
  if (!zero(a, 0) || !zero(b, 5)) return 3;
  return n;
}

template <class T> int nested(int n) {
  O o{};
  O p{{4}};
  H h{};
  H k{9};
  I i{3};
  D x{};
  D y{1};
  if (!zero(o.s, 0) || !zero(p.s, 4)) return 4;
  if (h.tag != 0 || !zeros(h.arr.data, 108) || k.tag != 9 || !zeros(k.arr.data, 108)) return 5;
  if (i.a != 3 || !zeros(i.b, 4)) return 6;
  if (x.a != 7 || y.a != 1 || !zeros(x.p, 3) || !zeros(y.p, 3)) return 7;
  return n;
}

template <class T> int arrays(int n) {
  S arr[2]{};
  S brr[2] = {{1}};
  S crr[] = {{1}, {2}};
  if (!zero(arr[0], 0) || !zero(arr[1], 0)) return 8;
  if (!zero(brr[0], 1) || !zero(brr[1], 0)) return 9;
  if (sizeof(crr) != 2 * sizeof(S) || !zero(crr[0], 1) || !zero(crr[1], 2)) return 10;
  return n;
}

template <class T> int assign(int n) {
  S a{1, {1, 1}};
  a = {2};
  S b = S{};
  if (!zero(a, 2) || !zero(b, 0)) return 11;
  return n;
}

// The shape the brief reduces from: a lambda in a function template.
template <class F> int call(F f) { return f(); }
template <class T> int in_lambda(int n) {
  return call([&] {
    S addr{};
    addr.fam = 1;
    return zero(addr, 1) ? n : 12;
  });
}

template <class T> struct C {
  int member(int n) {
    S a{};
    return zero(a, 0) ? n : 13;
  }
};

// Constant evaluation of the instantiated initializer.
template <class T> constexpr int folded() {
  S a{};
  O o{{2}};
  return a.path[107] + o.s.path[0] + o.s.fam;
}
static_assert(folded<int>() == 2, "");

int run_all() {
  int r = braces<int>(0);
  if (r == 0) r = dependent<S>(0);
  if (r == 0) r = nested<int>(0);
  if (r == 0) r = arrays<int>(0);
  if (r == 0) r = assign<int>(0);
  if (r == 0) r = in_lambda<int>(0);
  if (r == 0) r = C<int>().member(0);
  return r;
}
