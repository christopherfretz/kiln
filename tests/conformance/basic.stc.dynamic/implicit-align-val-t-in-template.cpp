// [basic.stc.dynamic.general]/2, [new.syn]: the first new-expression of the
// translation unit implicitly declares the global allocation functions and
// std::align_val_t. When that new-expression is inside a template (libc++'s
// `requires { ::new T; }` in <concepts>), the implicit std::align_val_t is
// still an ordinary, non-dependent enumeration: a later
// `operator new(std::size_t, std::align_val_t)` does not accept a nothrow_t.
//@ expect: compile-ok
template <class T> concept default_new = requires { ::new T; };

typedef decltype(sizeof 0) size_t;
namespace std {
struct nothrow_t { explicit nothrow_t() = default; };
extern const nothrow_t nothrow;
enum class align_val_t : size_t {};
}
void* operator new(size_t, const std::nothrow_t&) noexcept;
void* operator new(size_t, std::align_val_t);
void* operator new(size_t, std::align_val_t, const std::nothrow_t&) noexcept;

void* a = ::operator new(4, std::nothrow);
void* b = ::operator new(4, std::align_val_t(16), std::nothrow);
void* c = ::operator new(4, std::align_val_t(16));

template <class T> T* get(long n) {
  return static_cast<T*>(::operator new(n * sizeof(T), std::nothrow));
}
int* d = get<int>(3);
static_assert(__is_enum(std::align_val_t));
static_assert(!__is_convertible(std::nothrow_t, std::align_val_t));
