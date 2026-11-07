// [temp.constr.normal]/1.5.3 (P2841R7 with P2963R3): a fold over a concept
// template parameter pack whose parameter mapping supplies N concepts
// normalizes as the expanded `E0 && ... && EN-1` (or `||`), so the argument
// concepts' atoms take part in subsumption: all_of<T, B, D> subsumes
// all_of<T, A, C> when B and D subsume A and C ([temp.constr.normal]
// Example 4), through a forwarding concept too, and for `||` the
// disjunction of the weaker concepts is the more constrained one. clang 22
// does not expand the fold here and reports the calls ambiguous; kiln
// follows the wording (spec/sema-templates.md §8.2 "Wording over clang").
// WP-746.
//@ expect: run
//@ strict-diags
//@ skip(clang): clang 22 does not expand concept-pack folds in subsumption ([temp.constr.normal] Example 4 is ambiguous there)
//@ requires: clang>=22
//@ oracle: none
template <typename T> concept A = true;
template <typename T> concept C = true;
template <typename T> concept B = A<T> && true;
template <typename T> concept D = C<T> && true;
template <typename T> concept Never = false;
template <typename T, template <typename> concept... CTs>
concept all_of = (CTs<T> && ...);
template <typename T, template <typename> concept... CTs>
concept any_of = (CTs<T> || ...);
template <typename T, template <typename> concept... CTs>
concept fwd = all_of<T, CTs...>;

static_assert(all_of<int, A, C> && !all_of<int, A, Never> && all_of<int>);
static_assert(any_of<int, Never, A> && !any_of<int>);

// Example 4.
template <typename T> requires all_of<T, A, C> constexpr int f(T) { return 1; }
template <typename T> requires all_of<T, B, D> constexpr int f(T) { return 2; }
static_assert(f(1) == 2);
// Through a concept that forwards its pack.
template <typename T> requires fwd<T, A, C> constexpr int g(T) { return 1; }
template <typename T> requires fwd<T, B, D> constexpr int g(T) { return 2; }
static_assert(g(1) == 2);
// Disjunction: B || D implies A || C.
template <typename T> requires any_of<T, B, D> constexpr int k(T) { return 1; }
template <typename T> requires any_of<T, A, C> constexpr int k(T) { return 2; }
static_assert(k(1) == 1);

int main() { return f(1) + g(1) + k(1) == 5 ? 0 : 1; }
