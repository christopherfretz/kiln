// [over.match.class.deduct]/3: copy-list-initialization that selects an
// explicit guide is ill-formed; in copy-initialization explicit guides
// are not candidates.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:3 error "class template argument deduction for 'E' selected an explicit constructor for copy-list-initialization"
//@ diag: 15:39 note "explicit constructor declared here"
//@ diag: 19:3 error "class template argument deduction for 'G' selected an explicit deduction guide for copy-list-initialization"
//@ diag: 17:10 note "explicit deduction guide declared here"
//@ diag: 20:3 error "no viable constructor or deduction guide for deduction of template arguments of 'E'"
//@ diag: 15:26 note "candidate template ignored: could not match 'E<T>' against 'int'"
//@ diag: 15:26 note "implicit deduction guide declared as 'template <class T> E(E<T>) -> E<T>'"
//@ diag: 15:39 note "explicit constructor is not a candidate"
//@ diag: 15:39 note "implicit deduction guide declared as 'template <class T> explicit E(T) -> E<T>'"
template<class T> struct E { explicit E(T); };
template<class T> struct G { G(T); };
explicit G(int) -> G<int>;
E a = {1};
G b = {1};
E c = 1;
