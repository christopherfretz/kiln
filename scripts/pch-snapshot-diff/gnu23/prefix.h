/* A C project prefix: libc headers and project-style declarations. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Tentative definitions: completed (zero-initialized) at the end of the
   translation unit that uses the prefix, unless it defines them. */
int pfx_counter;
int pfx_counter;
static int pfx_hidden[];
uint32_t pfx_table[4];

/* C23: `()` is `(void)`; constexpr objects, typeof, attributes. */
int pfx_old(int);
[[nodiscard]] static int pfx_add(int a, int b) { return a + b; }
constexpr int pfx_limit = 64;
typedef typeof(pfx_limit + 1L) pfx_long_t;
static const bool pfx_on = true;

static_assert(sizeof(uint32_t) == 4);

static inline int pfx_twice(int x) { return pfx_add(x, x); }

struct pfx_point {
  int x, y;
};
enum pfx_color { PFX_RED, PFX_GREEN = 4, PFX_BLUE };

#pragma pack(push, 1)
struct pfx_packed {
  char c;
  int i;
};
#pragma pack(pop)

typedef struct pfx_point pfx_point_t;
extern size_t pfx_n;
#define PFX_MAX(a, b) ((a) > (b) ? (a) : (b))

static inline size_t pfx_len(const char *s) { return strlen(s); }
