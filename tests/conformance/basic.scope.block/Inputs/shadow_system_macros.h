// Found through -isystem: a system header. The nested statement
// expressions mirror the NEON intrinsic macros of <arm_neon.h>, whose
// inner `__ret` hides the outer one.
#define SYS_INNER(x) __extension__({ int __ret = (x); __ret; })
#define SYS_OUTER(x) __extension__({ int __ret = SYS_INNER(x); __ret; })
#define SYS_DECLARE(name) int name = 0; (void)name
#define SYS_PASTE_DECL(a, b) int a##b = 0; (void)a##b
#define SYS_LAMBDA_SHADOW() ([] { int __loc = 1; return __loc; }())
