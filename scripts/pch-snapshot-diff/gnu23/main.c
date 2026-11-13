size_t pfx_n = 3;
int pfx_old(int n) { return n + (pfx_on ? 1 : 0); }
static int pfx_hidden[2] = {5, 6};

int vla_sum(int n) {
  typedef int row_t[n];
  row_t r;
  int s = 0;
  for (int i = 0; i < n; i++) {
    r[i] = i;
    s += r[i];
  }
  return s + (int)sizeof(row_t);
}

int main(void) {
  pfx_point_t p = {1, 2};
  struct pfx_packed k = {'a', 7};
  char buf[32];
  pfx_counter++;
  pfx_table[1] = (uint32_t)pfx_twice(p.x);
  snprintf(buf, sizeof buf, "%d %d", PFX_MAX(p.x, p.y), PFX_BLUE);
  void *m = malloc(pfx_len(buf) + sizeof(struct pfx_packed));
  memcpy(m, &k, sizeof k);
  free(m);
  printf("%s %u %d %d %zu\n", buf, (unsigned)pfx_table[1], pfx_old(1), vla_sum(3) + pfx_hidden[1],
         sizeof(struct pfx_packed));
  pfx_long_t big = pfx_limit;
  int *none = nullptr;
  return pfx_counter - 1 + (none != nullptr) + (int)(big - pfx_limit);
}
