// [over.match.ref]/1, [over.match.copy]/1, [temp.constr.decl]: an
// explicit conversion function is not a candidate for a copy-initialization,
// so its constraints are never checked there. Binding `It&&` to a `V`
// (for `It`'s move constructor, while `It(*this)` is resolved inside
// `begin`'s deduced return type) considers `V`'s conversion functions;
// the explicit `operator bool` is dropped before its constraint `C<V>`
// would recurse into the check still in progress. The shape is a view
// interface's `explicit operator bool` and an iterator built from the
// view. Found running GCC's C++ testsuite (a `views::cache_latest`
// range).
//@ expect: compile-ok
//@ strict-diags
template <class T> concept C = requires(T& t) { t.begin(); };

template <class D> struct VI {
  constexpr explicit operator bool() requires C<D> { return true; }
};

struct V : VI<V> {
  struct It {
    V* p;
    constexpr explicit It(V& v) : p(&v) {}
    It(It&&) = default;
  };
  constexpr auto begin() { return It(*this); }
};

static_assert(C<V>);
static_assert(static_cast<bool>(V{}));
