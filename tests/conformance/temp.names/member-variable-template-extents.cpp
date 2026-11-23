// [temp.names]/1, [expr.ref]/1: a member access naming a static data
// member template's specialization ends at the `>` of its template
// argument list (spec/ast.md §21.2), outside a template and in one.
//@ expect: compile-ok
//@ std: c++20
struct V {
  template <int N> static constexpr int twice = 2 * N;
};
int k(int);
int use(V d, V *p) { return k(d.twice<7>) + k(p->twice<2>) + k(d.V::twice<3>); }
template <class T> int tuse(T d, V *p) { return k(d.template twice<7>) + k(p->twice<2>); }
int t = tuse(V{}, nullptr);
