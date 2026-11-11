// The translation unit of the `c` case: it uses bench/pch/c.h, which the
// bench supplies by `-include` or `-include-pch`.
int bench_use(const char *s) {
  char b[64];
  strncpy(b, s, sizeof b - 1);
  b[63] = 0;
  unsigned long c = crc32(0L, (const Bytef *)b, (uInt)strlen(b));
  printf("%s %lu\n", zlibVersion(), c);
  return atoi(b) + (int)(c & 1);
}
