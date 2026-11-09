// [temp.explicit]/3, /10, [dcl.array]/3: an explicit instantiation
// definition of a variable template specialization whose pattern has an
// array of unknown bound instantiates the initializer before the declared
// type is compared, so the declaration names the bound the initializer
// gives (clang `ActOnExplicitInstantiation` calls
// `InstantiateVariableDefinition` first). WP-907.
//@ expect: compile-ok
//@ strict-diags
template <class T> T arr[] = {1, 2};
template short arr<short>[2];
static_assert(sizeof(arr<short>) == 2 * sizeof(short));
template <class... T> unsigned long sizes[] = {sizeof(T)...};
template unsigned long sizes<char, int, long>[3];
static_assert(sizeof(sizes<char, int, long>) == 3 * sizeof(unsigned long));
