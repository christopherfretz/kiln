// __type_pack_element<I, Ts...> with a non-pack argument list
// (spec/sema-templates.md §9.3): the I-th type. The pack-expansion form
// is WP-072's, so __has_builtin stays false until then.
//@ strict-diags
static_assert(__is_same(__type_pack_element<0, int, char, long>, int));
static_assert(__is_same(__type_pack_element<2, int, char, long>, long));
static_assert(__is_same(__type_pack_element<1, int, const char &>, const char &));
static_assert(!__is_same(__type_pack_element<1, int, char>, int));
template <unsigned long I> struct Pick { typedef __type_pack_element<I, short, double> type; };
static_assert(__is_same(Pick<1>::type, double));
