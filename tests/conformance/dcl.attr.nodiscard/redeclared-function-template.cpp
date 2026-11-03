// [dcl.attr.nodiscard]/2, [temp.inst] (WP-491): a nodiscard attribute on
// any declaration of a function template seen before a specialization is
// formed applies to calls of that specialization, as clang substitutes it
// from the latest redeclaration, which carries every earlier one's
// attributes. libc++ shapes: `get` and the valarray `begin` forward-declared
// without the attribute and defined with it; `any_cast` forward-declared,
// befriended, then defined with it; a forward declaration after an
// attributed definition. A specialization formed before the attributed
// declaration keeps what it had (clang does not warn on its `get<1>`). An
// explicit specialization is diagnosed as a redeclaration of the template.
// `pure` and `const` merge the same way (clang `warn_unused_call`).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 52:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 53:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 54:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 55:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 56:3 warning "ignoring return value of function declared with 'nodiscard' attribute: msg"
//@ diag: 57:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 58:3 warning "ignoring return value of function declared with pure attribute"
//@ diag: 47:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 59:3 note "in instantiation of function template specialization 'dependent<int>' requested here"
using size_t = decltype(sizeof 0);
template <class T, size_t N> struct arr { T e[N]; };
template <class T> struct valarr { T* p; };
struct any { int* p; };

template <size_t I, class T, size_t N> T& get(arr<T, N>&) noexcept;
template <class T> T* begin(valarr<T>&);
template <class V> V* any_cast(any*) noexcept;
struct holder {
  template <class V> friend V* any_cast(any*) noexcept;
};
inline int early(arr<int, 2>& a) { return get<1>(a); }

template <size_t I, class T, size_t N> [[nodiscard]] T& get(arr<T, N>& a) noexcept { return a.e[I]; }
template <class T> [[nodiscard("msg")]] T* begin(valarr<T>& v) { return v.p; }
template <class V> [[nodiscard]] V* any_cast(any* a) noexcept { return (V*)a->p; }
template <class T> [[nodiscard]] T id(T x) { return x; }
template <class T> T id(T x);
template <class T> [[nodiscard]] T twice(T x) { return x + x; }
template <> int twice<int>(int x) { return x * 2; }
template <class T> T pure_fn(T);
template <class T> [[gnu::pure]] T pure_fn(T x) { return x; }

template <class T> void dependent(T t) {
  id(t);
}

void use(arr<int, 2>& a, valarr<long>& v, any* p) {
  get<1>(a);
  get<0>(a);
  get<0>(static_cast<arr<int, 2>&>(a));
  any_cast<int>(p);
  id(1.0);
  begin(v);
  twice(1);
  pure_fn(1);
  dependent(1);
}
