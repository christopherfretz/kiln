// [dcl.init.list]/3.2: an aggregate class list-initialized from a single
// element of the same or a derived class type is initialized from that
// element, not by aggregate initialization of its first member (WP-151).
struct P { int a, b; };
struct D : P { int c; };
P u{1, 2};
P t1 = {u};
P t2 = {P{4}};
P t3 = {P(4, 5)};
P t4{u};
D d{};
P t5 = {d};
struct W { P p; int z; };
W w = {u, 3};
W w2 = {W{u, 4}};
constexpr P cp{5, 6};
constexpr P cq = {cp};
static_assert(cq.a == 5 && cq.b == 6);
constexpr P cr = {D{{7, 8}, 9}};
static_assert(cr.a == 7 && cr.b == 8);
constexpr W cw = {W{cp, 4}};
static_assert(cw.p.b == 6 && cw.z == 4);
template <class T> struct A {
  T l(T t = {T(4)}) { return t; }
};
P x = A<P>().l();
constexpr P f(P p) { return {p}; }
static_assert(f(cp).a == 5);
