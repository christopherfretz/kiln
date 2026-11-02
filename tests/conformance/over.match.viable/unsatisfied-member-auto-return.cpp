// [over.match.viable]/3, [expr.prim.id.general]/4, [temp.inst]/5: a
// member of a class template specialization whose trailing
// requires-clause is not satisfied is not viable, and naming it in a
// member access is ill-formed; its placeholder return type is never
// deduced, so its definition is never instantiated. In a requires-expression
// the requirement is simply unsatisfied (libc++ counted_iterator::operator->
// probed by iterator_traits).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:29 error "invalid reference to function 'foo': constraints not satisfied"
//@ diag: 18:48 note "because 'fwd' does not satisfy 'contig'"
//@ diag: 13:37 note "because 'fwd::contig' evaluated to false"
template <class T> concept contig = T::contig;
template <class T> T* to_addr(T* p) { return p; }

template <class I> struct counted {
  I cur;
  constexpr auto foo() const noexcept requires contig<I> { return to_addr(cur); }
  constexpr auto operator->() const noexcept requires contig<I> { return to_addr(cur); }
};
struct fwd { static constexpr bool contig = false; };
struct ptr { static constexpr bool contig = true; };

template <class I> concept has_foo = requires(I& i) { i.foo(); };
template <class I> concept has_arrow = requires(I& i) { i.operator->(); };
static_assert(!has_foo<counted<fwd>>);
static_assert(!has_arrow<counted<fwd>>);

template <class I> struct traits { using pointer = void; };
template <class I> requires requires(I& i) { i.operator->(); }
struct traits<I> { using pointer = decltype(static_cast<I&(*)()>(nullptr)().operator->()); };
static_assert(__is_same(traits<counted<fwd>>::pointer, void));

void g(counted<fwd>& c) { c.foo(); }
