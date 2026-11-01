// Designated initializer lists in function and class templates: omitted
// members zero-initialized or taking their default member initializers,
// nested and anonymous-member designators, array designators with holes,
// a designated pack expansion, and a std::array-like wrapper.
struct Pt { int x; int y; int z; };
struct Q { Pt p; int w = 9; };
template<class T> struct Pair { T first; T second; long tag = 3; };
template<class T, unsigned N> struct Arr { T elems[N]; };
template<class T> struct V { int m; union { T u; int i; }; T r; };

template<class T> T make_y(int v) { return T{.y = v}; }
template<class P, class T> P pair_second(T v) { return {.second = v}; }
template<class T> T nested(int v) { T t{.p.y = v, .w = 2}; return t; }
template<class R, class T> R anon(T v) { return {.m = 1, .u = v}; }
template<class R, unsigned N, class T> R arr(T v) { return {.elems = {[1] = v, [N - 1] = v}}; }
template<class... U> Pt pack(U... u) { return Pt{.x = u...}; }
template<class T> int local(int v) {
  struct S { int m; union { T u; int i; }; } s{.m = 2, .u = T(v)};
  return s.m + s.u;
}
template<class T> struct Holder {
  T t{.x = 1, .z = 3};
  int get() const { return t.x + t.z; }
};

int use(int v) {
  Pt a = make_y<Pt>(v);
  Pair<long> b = pair_second<Pair<long>>(long(v));
  Q c = nested<Q>(v);
  V<double> d = anon<V<double>>(2.5);
  Arr<short, 4> e = arr<Arr<short, 4>, 4>(short(v));
  Pt f = pack(v);
  Holder<Pt> h;
  return a.y + int(b.second + b.tag) + c.p.y + c.w + d.m + int(d.u) + e.elems[3]
      + f.x + local<int>(v) + h.get();
}
