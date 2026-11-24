// The module of subst-written-module-run.cpp.
export module subst_written_mod;
export namespace n { struct S { static long s() { return 0; } }; }
export template <class T> struct TB {
  T dep[sizeof(T::s())];
  char kept[sizeof(sizeof(T::s()))];
};
export inline int tb_size() {
  TB<n::S> tb{};
  return sizeof(tb.dep) + sizeof(tb.kept);
}
export template <class T> inline int tf(T, char tp[sizeof(T::s())]) {
  return sizeof(T::s()) + (tp == nullptr);
}
export inline int tf_use() { return tf(n::S(), nullptr); }
