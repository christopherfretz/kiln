// [expr.new]/28: a conditional new-expression whose placement arguments
// are class objects; the deallocation cleanup passes the saved aggregate
// arguments (clang `DominatingValue<RValue>::save` of an aggregate).
typedef decltype(sizeof 0) size_t;
struct Tag { int a; };
struct Big { long a[5]; };
struct NT { NT(); NT(const NT&); ~NT(); int x; };
void *operator new(size_t, Tag);
void operator delete(void *, Tag);
void *operator new(size_t, Big, int);
void operator delete(void *, Big, int);
void *operator new(size_t, NT);
void operator delete(void *, NT);
struct T { T(); T(int); ~T(); };
T *f1(bool c) { return c ? new (Tag{1}) T : nullptr; }
T *f2(bool c, Big b) { return c ? new (b, 3) T(2) : nullptr; }
T *f3(bool c, Tag t) { return c ? nullptr : new (t) T; }
T *f4(bool c) { return c ? new (NT{}) T : nullptr; }
T *f5(bool c, bool d) { return c ? (d ? new (Tag{2}) T : new (Big{}, 1) T) : nullptr; }
