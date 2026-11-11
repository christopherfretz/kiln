// A member deduction guide of a member class template (spec/modules.md
// §11.11 "Deduction guides", WP-1292): the declaration image refuses it
// ("a member deduction guide") and the module falls back to replay.
export module imgmg;
export namespace mg {
struct O {
  template <class T> struct I {
    template <class U> I(U *) : v(sizeof(T)) {}
    int v;
  };
  template <class T> I(T *) -> I<T[2]>;
};
} // namespace mg
