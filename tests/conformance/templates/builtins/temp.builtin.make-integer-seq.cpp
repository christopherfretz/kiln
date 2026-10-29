// __make_integer_seq<Seq, T, N> (spec/sema-templates.md §9.3): names
// Seq<T, 0, 1, ..., N-1>, as libc++'s make_integer_sequence uses it.
//@ strict-diags
template <class T, T... Is> struct Seq {};
static_assert(__is_same(__make_integer_seq<Seq, int, 3>, Seq<int, 0, 1, 2>));
static_assert(__is_same(__make_integer_seq<Seq, unsigned long, 0>, Seq<unsigned long>));
static_assert(__is_same(__make_integer_seq<Seq, long, 5>, Seq<long, 0, 1, 2, 3, 4>));
static_assert(!__is_same(__make_integer_seq<Seq, int, 2>, Seq<int, 1, 0>));
template <class T, T N> using MakeSeq = __make_integer_seq<Seq, T, N>;
static_assert(__is_same(MakeSeq<short, 2>, Seq<short, 0, 1>));
