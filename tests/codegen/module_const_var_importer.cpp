// An importer's declaration of a const namespace-scope variable that an
// imported module unit defines (not inline, not an instantiation) and
// whose value is a constant is `available_externally` with that value,
// as clang 22 emits it (`GetOrCreateLLVMGlobal`: no definition in this
// unit, an initializer): constexpr and const scalars, pointers, arrays,
// an explicit specialization of a variable template, class objects with
// constant destruction, `constinit const`, `volatile const`. A mutable
// member, a non-constant destructor or a non-const variable keep an
// `external` declaration; an inline one or an implicit instantiation is
// the importer's own `linkonce_odr` copy. A reference's lifetime-extended
// temporary is the module unit's symbol, `available_externally` here
// (WP-1653).
//@ module: mod_const_vars=Inputs/mod_const_vars.cppm
import mod_const_vars;
int use() {
  return *&c1 + *&c2 + *&c3 + **&c4 + *&vt<int> + *&vt<long> + (&c5)->v + (&c6)->v +
         *&arr[1] + *&c8 + g1 + (&d1)->v + (&d2)->v + *&d4 + (int)*&d7 + *&d9 + d10 +
         (&e5)->v + (&e6)->v;
}
