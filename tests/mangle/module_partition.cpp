// [module.unit]/3, Itanium ABI <module-name>: entities of a partition
// carry the primary module's name only (`W1M`); the partition's
// initializer is `_ZGIW1MWP4Part`.
export module M:Part;
export int pf() { return 1; }
int pg() { return 2; }
export namespace pn {
template <class T> struct Box {
  T v;
  T get() const { return v; }
};
int use() { return Box<int>{3}.get() + pg(); }
} // namespace pn
