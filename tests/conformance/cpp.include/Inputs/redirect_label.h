// glibc's __REDIRECT shape: a prototype, then a redeclaration that names
// its symbol with an asm label (the label belongs to the entity).
extern "C" int redirected_add(int, int);
extern "C" int redirected_add(int, int) __asm__("redirect_target_add");
namespace rl {
int twice(int);
int twice(int) __asm__("redirect_target_twice");
inline int both(int x) { return redirected_add(x, 1) + twice(x); }
} // namespace rl
