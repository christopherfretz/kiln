// __TIMESTAMP__ (the GNU extension beside [cpp.predefined]/1's __DATE__
// and __TIME__) without a fixed epoch is the current file's modification
// time in local time, spelled as asctime does: "Ddd Mmm dd hh:mm:ss yyyy",
// the day of the month padded with a space (WP-2331, spec/pp.md "Builtin
// macros"). The checks are on the shape only, so they hold for any clock
// and zone (and under a fixed epoch too); the header's own expansion is
// checked as well.
//@ expect: compile-ok
//@ strict-diags
#include "Inputs/timestamp.h"

constexpr bool digit(char c) { return c >= '0' && c <= '9'; }

// `s` starts with one of the three-letter names in `list`.
constexpr bool named(const char *s, const char *list) {
  for (; *list; list += 3)
    if (s[0] == list[0] && s[1] == list[1] && s[2] == list[2])
      return true;
  return false;
}

constexpr int num(const char *s) { return (s[0] - '0') * 10 + (s[1] - '0'); }

constexpr bool well_formed(const char *s) {
  int n = 0;
  while (s[n])
    ++n;
  if (n != 24)
    return false;
  if (!named(s, "SunMonTueWedThuFriSat") || s[3] != ' ')
    return false;
  if (!named(s + 4, "JanFebMarAprMayJunJulAugSepOctNovDec") || s[7] != ' ')
    return false;
  if (!(s[8] == ' ' || (s[8] >= '1' && s[8] <= '3')) || !digit(s[9]) || s[10] != ' ')
    return false;
  int day = (s[8] == ' ' ? 0 : s[8] - '0') * 10 + (s[9] - '0');
  if (day < 1 || day > 31)
    return false;
  for (int i = 11; i < 19; i += 3)
    if (!digit(s[i]) || !digit(s[i + 1]))
      return false;
  if (s[13] != ':' || s[16] != ':' || s[19] != ' ')
    return false;
  if (num(s + 11) > 23 || num(s + 14) > 59 || num(s + 17) > 60)
    return false;
  for (int i = 20; i < 24; ++i)
    if (!digit(s[i]))
      return false;
  return true;
}

static_assert(sizeof(__TIMESTAMP__) == 25);
static_assert(well_formed(__TIMESTAMP__));
static_assert(well_formed(header_stamp));
// Not the spelling of an unknown time.
static_assert(__TIMESTAMP__[0] != '?');
