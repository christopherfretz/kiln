// List-initialization of vectors (clang InitListChecker::CheckVectorType,
// C++ rules): a leading vector initializes the whole vector, otherwise
// one scalar per element with the rest zero, `{}` is a zero vector, and
// excess elements are an error (WP-156).
//@ expect: compile-fail
//@ diag: 23:22 error "excess elements in vector initializer"
//@ diag: 24:13 error "excess elements in vector initializer"
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v2f __attribute__((ext_vector_type(2)));
typedef int g4i __attribute__((vector_size(16)));

v4i a = {1, 2, 3, 4};
v4i b{1, 2};
v4i c = {};
v4i d = {a};
v4i e = v4i{a};
v2f f = {1.5f, 2};
g4i g = {1, 2, 3};
template <class... T> v4i mk(T... t) { return v4i{t...}; }
v4i h = mk(1, 2, 3);
static_assert(sizeof(v4i{1}) == 16);

v4i x = {1, 2, 3, 4, 5};
g4i y = {g, 1};
