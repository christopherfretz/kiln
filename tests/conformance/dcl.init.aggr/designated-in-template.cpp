// [dcl.init.aggr]/3.1, [temp.dep.expr]/3: a designated braced-init-list
// whose type or clauses are dependent keeps its designators until
// instantiation, in a functional cast, a return, a copy-list-init and a
// mem-initializer (libc++ <__format/parser_std_format_spec.h>:
// `__parsed_specifications<_CharT>{.__std_ = __std{...}, .__width_{...}}`).
//@ expect: run
//@ strict-diags
struct In { int a, b; };
template <class T> struct Out { In in; T w; T p{7}; int arr[3]; };
union U { int i; float f; };
template <class T> Out<T> mk(T x) {
  return Out<T>{.in = In{.a = 1, .b = 2}, .w{x}};
}
template <class T> Out<T> mk2(T x) { return {.in{.b = 5}, .w = x + 1}; }
template <class T> T un(T) { U u{.f = T(2.5)}; return T(u.f); }
template <class T> struct M { In in; M(T t) : in{.a = int(t), .b = 3} {} };
int main() {
  auto o = mk(3);
  auto o2 = mk2(4);
  M<long> m(8);
  return (o.in.a == 1 && o.in.b == 2 && o.w == 3 && o.p == 7 && o2.in.a == 0 &&
          o2.in.b == 5 && o2.w == 5 && un(1.0) == 2.5 && m.in.a == 8 && m.in.b == 3) ? 0 : 1;
}
