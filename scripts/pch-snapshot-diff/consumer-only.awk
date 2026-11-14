# The consumer-only diagnostics of a replay compile (WP-1394,
# spec/pch.md "Consumer"): the replay's stderr without the
# header's own diagnostics, which a snapshot consumer does not show
# again. A diagnostic is its primary line (`file:line:col:
# warning|error|fatal error|remark:`), the include stack before it and
# every line after it up to the next one (its source lines and notes).
# A replay reports the header's own first, so the first `n` are dropped:
# `n` is what the restored engine says the header reported, which the
# snapshot consumer prints under KILN_SNAP_TIMING (`snapshot-timing:
# the header reported N diagnostic(s)`). The `N warnings and M errors
# generated.` line is recounted without the dropped ones, in clang's
# wording.
#
#   awk -v n=N -f consumer-only.awk replay.diag
function norm(l) { sub(/^\.\//, "", l); return l }
function primary(l) {
  return l ~ /^[^ ].*: (warning|error|fatal error|remark): /
}
function kind(l) {
  if (l ~ /: warning: /) return "w"
  if (l ~ /: (error|fatal error): /) return "e"
  return ""
}
function file_of(l) { l = norm(l); sub(/:.*/, "", l); return l }
# A dropped diagnostic's include stack is printed before the next kept
# one in the same file instead (clang prints the stack when the file
# changes).
function flush_block() {
  if (nblock == 0) return
  if (drop) {
    if (nstack > 0) {
      ncarry = nstack
      for (i = 1; i <= nstack; i++) carry[i] = block[i]
      carry_file = file_of(block[nstack + 1])
    }
  } else {
    if (ncarry > 0 && nstack == 0 && file_of(block[1]) == carry_file)
      for (i = 1; i <= ncarry; i++) print carry[i]
    ncarry = 0
    for (i = 1; i <= nblock; i++) print block[i]
  }
  nblock = 0
  nstack = 0
  drop = 0
}
function plural(n, w) { return n " " w (n == 1 ? "" : "s") }
/ generated\.$/ {
  flush_block()
  w = 0; e = 0
  if (match($0, /[0-9]+ warnings?/)) w = substr($0, RSTART, RLENGTH) + 0
  if (match($0, /[0-9]+ errors?/)) e = substr($0, RSTART, RLENGTH) + 0
  w -= dw; e -= de
  if (w > 0 && e > 0) print plural(w, "warning") " and " plural(e, "error") " generated."
  else if (w > 0) print plural(w, "warning") " generated."
  else if (e > 0) print plural(e, "error") " generated."
  next
}
/^In file included from / {
  if (!stack) flush_block()
  stack = 1
  block[++nblock] = $0
  nstack++
  next
}
{
  if (primary($0)) {
    if (!stack) flush_block()
    stack = 0
    if (dropped < n + 0 && kind($0) != "") {
      dropped++
      drop = 1
      t = kind($0)
      if (t == "w") dw++
      if (t == "e") de++
    }
  } else if (/^                 from / && stack) {
    nstack++
  } else {
    stack = 0
  }
  block[++nblock] = $0
}
END { flush_block() }
