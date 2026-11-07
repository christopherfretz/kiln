// [temp.constr.normal]/1.4.1-1.4.2 (P2841R7): a concept-id whose concept is
// a concept template parameter is a concept-dependent constraint; once the
// parameter mapping supplies the concept, the normal form of `CC<U*, C>`
// (`CC = CT<T>`) is the normal form of `C<U*>`, so its atoms are C's atoms
// and take part in subsumption ([temp.constr.order]/2). [temp.constr.normal]
// Example 3; clang 22 agrees (spec/sema-templates.md §8.2). WP-746.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
template <typename T> concept C = true;
template <typename T> concept CP = true;
template <typename T, template <typename> concept CT> concept CC = CT<T>;
template <typename T> concept Small = sizeof(T) <= 8;

// CC<U*, C> normalizes to C's atom `true` with T -> U*.
template <typename U> constexpr int g() requires CC<U*, C> { return 1; }
template <typename U> constexpr int g() requires CC<U*, C> && CP<U> { return 2; }
static_assert(g<int>() == 2);
template <typename U> constexpr int h() requires C<U*> { return 1; }
template <typename U> constexpr int h() requires CC<U*, C> && CP<U> { return 2; }
static_assert(h<int>() == 2);

// Satisfaction goes through the argument concept.
template <typename T, template <typename> concept CT> concept Ptr = CT<T*>;
static_assert(Ptr<int, Small> && Ptr<char, C>);
template <typename T> constexpr int k() requires Ptr<T, Small> { return 1; }
template <typename T> constexpr int k() requires Small<T*> && CP<T> { return 2; }
static_assert(k<int>() == 2);
