// clang `EmitAggregateCopy` ignores empty classes: the trivial copy or move
// of an empty base stores nothing, so it cannot clobber the member that
// shares its address under the empty base optimization ([intro.object]/8;
// libc++'s `__compressed_pair_elem<D, 1, true>` beside a pointer, WP-164).
struct Empty {};
struct Elem : Empty {
  Elem(const Empty &e) noexcept : Empty(e) {}
  Elem(Empty &&e) noexcept : Empty(static_cast<Empty &&>(e)) {}
};
struct PtrElem {
  int *p;
};
struct Pair : PtrElem, Elem {
  Pair(int *p, Empty e) noexcept : PtrElem{p}, Elem(e) {}
};
int *pair(int *p) { return Pair(p, Empty{}).p; }

struct Holder : Empty {
  int *p;
};
Holder gh;
Empty ge;
void assign() noexcept { static_cast<Empty &>(gh) = ge; }
void move_assign() noexcept { static_cast<Empty &>(gh) = static_cast<Empty &&>(ge); }
