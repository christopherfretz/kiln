// [over.over]/3 with [temp.arg.explicit]/4: a function template-id that is
// not called names its one specialization when every template argument is
// explicit, also when the arguments are dependent and only substituted at
// instantiation (libc++ insert_node_type_hint.pass.cpp
// `auto* nf = &node_factory<Container>;`), so `auto` deduces the function
// (pointer) type instead of "incompatible initializer of type
// '<overloaded function type>'".
//@ expect: run
//@ strict-diags
template <class C> C make(int k) { return C(k + 1); }
template <class C> int use() {
  auto *p = &make<C>;
  auto q = &make<C>;
  auto *r = make<C>;
  auto &f = make<C>;
  static_assert(__is_same(decltype(p), C (*)(int)));
  static_assert(__is_same(decltype(f), C (&)(int)));
  return p(1) + q(2) + r(3) + f(4);
}
int main() { return use<int>() == 2 + 3 + 4 + 5 && use<long>() == 14 ? 0 : 1; }
