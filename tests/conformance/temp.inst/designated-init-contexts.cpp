// [temp.inst], [dcl.init.aggr]/3.1: designated initializer lists in templates:
// dependent designated init of a dependent aggregate, a deduced return, CTAD, a
// default member initializer, a default argument, an NTTP argument, a
// requires-expression (an unknown field is a substitution failure), a designated
// pack expansion, nested designators, and an anonymous union member of a local class.
//@ skip(gcc): relies on clang's C99 designator extensions (nested designators, a designated pack expansion)
//@ expect: run
//@ strict-diags
//@ diag: 20:41 warning "nested designators are a C99 extension"
struct Pt { int x; int y; int z; };
template<class T> struct Agg { T a; T b; };
template<class T> struct Box { T v{.x = 1, .z = 3}; int n = 0; };
template<class T> int defarg(T t = {.y = 2}) { return t.y; }
template<Pt P> constexpr int nttp() { return P.x + P.z; }
template<class T> concept HasX = requires { T{.x = 1}; };
template<class T> concept HasQ = requires { T{.q = 1}; };
template<class T, class... U> T mk(U... u) { return T{.x = u...}; }
template<class T> T sub(int v) { return {.y = v, .z = v + 1}; }
template<class T> auto ctad() { return Agg{.a = T(1), .b = T(2)}; }
template<class T> T nested(int v) { T t{.p.y = v, .w = 2}; return t; }
struct Q { Pt p; int w; };
template<class T> int anon(int v) { struct { int m; union { T u; int i; }; } s{.u = T(v)}; return s.u; }
int use() {
  Box<Pt> b;
  static_assert(HasX<Pt> && !HasQ<Pt> && !HasX<int>);
  static_assert(nttp<Pt{.x = 1, .z = 4}>() == 5);
  return defarg<Pt>() + sub<Pt>(3).z + ctad<long>().b + nested<Q>(4).p.y + b.v.z + anon<int>(3) + mk<Pt>(1).x;
}
int main() { return use() == 2 + 4 + 2 + 4 + 3 + 3 + 1 ? 0 : 1; }
