// [stmt.expr], [over.match.class.deduct] (WP-172 item 7): a functional
// cast whose class template arguments are deduced (`Pair(Empty{});`) is
// checked through its constructor call like any constructor conversion,
// so it does not warn; aggregate deduction and braced forms do.
//@ strict-diags
//@ diag: 24:3 warning "expression result unused [-Wunused-value]"
//@ diag: 25:3 warning "expression result unused [-Wunused-value]"
//@ diag: 27:3 warning "expression result unused [-Wunused-value]"
//@ diag: 28:3 warning "expression result unused [-Wunused-value]"
//@ diag: 29:3 warning "expression result unused [-Wunused-value]"
//@ diag: 30:3 warning "expression result unused [-Wunused-value]"
//@ diag: 33:3 warning "expression result unused [-Wunused-value]"
struct Empty {};
template <class T> struct Pair { T t; Pair(T x) : t(x) {} };
template <class T> struct Agg { T t; };
template <class T> Agg(T) -> Agg<T>;
struct NT { NT(Empty); };
struct TR { int x; };
struct E2 { Empty e; };
template <class T> struct PT { PT(T); ~PT(); };
void f() {
  Pair(Empty{});
  Pair<Empty>(Empty{});
  Agg(Empty{});
  Agg{Empty{}};
  NT(Empty{});
  TR{1};
  TR(1);
  E2(Empty{});
  E2{Empty{}};
  PT(Empty{});
  Empty();
  int(3);
}
