// [expr.type.conv]/2, [dcl.init.aggr], [temp.inst]: `T{x}` of an aggregate
// with a non-trivially destructible member, in a dependent call of a generic
// lambda, is rebuilt from its braced list when the lambda is instantiated
// (the list's temporary is bound under the functional cast; libc++
// `flat_map::find(Transparent<std::string>{key})`).
//@ expect: compile-ok
//@ strict-diags
struct Str { Str(const char*) {} Str(const Str&) {} ~Str() {} };
template <class T> struct Tr { T t; };
struct M { template <class K> int find(const K&) { return 0; } };
template <class U> void g() {
  auto f = [&](auto&& map, const Str& key) { return map.find(Tr<Str>{key}); };
  M m;
  f(m, "a");
}
template void g<int>();
