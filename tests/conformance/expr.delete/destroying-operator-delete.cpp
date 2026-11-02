// [expr.delete]/7, /10, [basic.stc.dynamic.deallocation]/5: a class's
// destroying `operator delete(C*, std::destroying_delete_t, ...)` is
// preferred over the usual ones, the destructor is not run by the
// delete-expression, and the operand is passed converted to `C*`.
//@ expect: run
//@ stdout: destroy 7 size 1
//@ stdout: delete 9
extern "C" int printf(const char*, ...);
extern "C" void free(void*);
extern "C" void* malloc(__SIZE_TYPE__);
namespace std {
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
struct T {
  int v;
  ~T() { printf("dtor\n"); }
  void* operator new(__SIZE_TYPE__ n) { return malloc(n); }
  void operator delete(void* p) { printf("usual\n"); free(p); }
  void operator delete(T* p, std::destroying_delete_t, __SIZE_TYPE__ n) {
    printf("destroy %d size %d\n", p->v, n == sizeof(T));
    free(p);
  }
};
struct U {
  int v;
  void operator delete(U* p, std::destroying_delete_t) {
    printf("delete %d\n", p->v);
    p->~U();
    ::operator delete(p);
  }
};
int main() {
  T* t = new T{7};
  delete t;
  U* u = new U{9};
  delete u;
  return 0;
}
