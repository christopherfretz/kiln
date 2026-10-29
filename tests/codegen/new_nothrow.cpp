// new (std::nothrow) and its placement delete ([new.delete.single]/5).
namespace std { struct nothrow_t { }; extern const nothrow_t nothrow; }
void *operator new(unsigned long, const std::nothrow_t &) noexcept;
void operator delete(void*, const std::nothrow_t &) noexcept;
int *a() { return new (std::nothrow) int(4); }
int *b(bool c) { return c ? new int(4) : nullptr; }
int *d(bool c) { return c && new int(4) ? nullptr : nullptr; }
