// [dcl.type.elab]/3, [temp.dep.type]/6: `class-key T::X` with a dependent
// qualifier is looked up when the template is instantiated: a missing tag
// is diagnosed then, and a friend declared that way befriends only the
// class it names (WP-332). Diagnostics as clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:40 error "no struct named 'Missing' in 'Q'"
//@ diag: 16:6 note "in instantiation of template class 'J<Q>' requested here"
//@ diag: 21:32 error "'v' is a private member of 'G<Q>'"
//@ diag: 17:80 note "declared private here"
struct Q {
  struct S { static int get(); };
};
struct R { struct S { static int get(); }; };
template<class T> struct J { struct T::Missing* p; };
J<Q> j;
template<class T> struct G { friend struct T::S; private: static constexpr int v = 7; };

int Q::S::get() { return G<Q>::v; }

int R::S::get() { return G<Q>::v; }
int main() {}
