"""Generates the kiln_pp integration tests with clang-19 goldens.

usage: python3 gen.py <cases module> ...   (e.g. `python3 gen.py cpp_replace`)

Each cases module (`cpp_*.py` here) lists `C(...)` cases; for each, clang is
run as `clang++-19 -std=c++26 -E -P` on the case's source (headers in
`inc/`, `sys/`, `next/`, resources in `res/`) and its output and
diagnostics become the golden. `want=` / `diags=` override a golden (a
recorded deviation; say why in `note=`). Writes `../<NAME>.rs` and runs
rustfmt on it. Needs clang-19 at /usr/bin/clang++-19.
"""
import importlib, os, re, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
WT = os.path.dirname(HERE)
WORK = os.path.join(tempfile.gettempdir(), 'kiln-pp-goldens-%d' % os.getpid())
DIAG = re.compile(r'^(\S+?):(\d+):(\d+): (warning|error|fatal error): (.*?)(?: \[-W[^\]]*\])?$')


class C:
    def __init__(self, name, tag, src, files=None, args=(), want=None, diags=None,
                 note=None, cfg=None):
        self.name, self.tag, self.src = name, tag, src
        self.files = files or {}
        self.args, self.want, self.diags, self.note, self.cfg = list(args), want, diags, note, cfg


def rs(s):
    if isinstance(s, bytes):
        return 'b"' + ''.join(chr(c) if 32 <= c < 127 and chr(c) not in '"\\' else '\\x%02x' % c for c in s) + '"'
    n = 1
    while ('"' + '#' * n) in s:
        n += 1
    return 'r' + '#' * n + '"' + s + '"' + '#' * n


def clang(c):
    if os.path.isdir(WORK):
        shutil.rmtree(WORK)
    os.makedirs(WORK)
    args = ['/usr/bin/clang++-19', '-std=c++26', '-E', '-P']
    dirs = {'h': 'inc', 's': 'sys', 'n': 'next', 'r': 'res'}
    for name, (kind, text) in c.files.items():
        p = os.path.join(WORK, dirs[kind], name)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, 'wb') as f:
            f.write(text if isinstance(text, bytes) else text.encode())
    args += ['-I', 'inc', '-I', 'next', '-isystem', 'sys', '--embed-dir=res']
    with open(os.path.join(WORK, 't.cpp'), 'w') as f:
        f.write(c.src)
    p = subprocess.run(args + c.args + ['t.cpp'], cwd=WORK, capture_output=True)
    diags = []
    for line in p.stderr.decode().splitlines():
        m = DIAG.match(line)
        if m:
            diags.append('%s:%s: %s: %s' % (m[2], m[3], m[4], m[5]))
    return p.stdout.decode(), diags


def resolver(c):
    meth = {'h': 'with_header', 's': 'with_system_header', 'n': 'with_next_header', 'r': 'with_resource'}
    out = '    let r = MemoryResolver::new()'
    for name, (kind, text) in c.files.items():
        if kind == 'r' and isinstance(text, str):
            text = text.encode()
        out += '\n        .%s("%s", %s)' % (meth[kind], name, rs(text))
    return out + ';\n'


def emit(mod):
    out = ['//! %s\n//!\n//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).\n' % mod.TITLE,
           '#![allow(\n    clippy::needless_raw_string_hashes,\n    clippy::doc_markdown,\n    reason = "generated goldens"\n)]\n\n',
           'mod common;\n\n']
    uses = set()
    body = []
    for c in mod.CASES:
        text, diags = clang(c)
        if c.want is not None:
            text = c.want
        if c.diags is not None:
            diags = c.diags
        b = ''
        tags = c.tag if isinstance(c.tag, list) else [c.tag]
        for t in tags:
            b += '/// %s\n' % t
        if c.note:
            b += '///\n'
            for l in c.note.splitlines():
                b += '/// %s\n' % l
        b += '#[test]\nfn %s() {\n' % c.name
        dl = '&[%s]' % ', '.join(rs(d) for d in diags)
        if c.files or c.cfg:
            if c.files:
                b += resolver(c)
                uses.add('MemoryResolver')
            else:
                b += '    let r = MemoryResolver::new();\n'
                uses.add('MemoryResolver')
            cfg = c.cfg or '|_| {}'
            uses.add('run_with')
            b += '    common::expect(\n        &common::run_with(%s, r, PrintOptions::default(), %s),\n        %s,\n        %s,\n    );\n' % (rs(c.src), cfg, rs(text), dl)
            uses.add('PrintOptions')
        elif diags:
            b += '    common::check_diags(\n        %s,\n        %s,\n        %s,\n    );\n' % (rs(c.src), rs(text), dl)
        else:
            b += '    common::check(\n        %s,\n        %s,\n    );\n' % (rs(c.src), rs(text))
        b += '}\n'
        body.append(b)
    kp = sorted(u for u in uses if u in ('MemoryResolver', 'PrintOptions'))
    if kp:
        out.append('use kiln_pp::{%s};\n\n' % ', '.join(kp) if len(kp) > 1 else 'use kiln_pp::%s;\n\n' % kp[0])
    out.append('\n'.join(body))
    with open(os.path.join(WT, mod.NAME + '.rs'), 'w') as f:
        f.write(''.join(out))
    subprocess.run(['rustfmt', '--edition', '2024', os.path.join(WT, mod.NAME + '.rs')], check=True)
    print('wrote', mod.NAME, len(mod.CASES))


if __name__ == '__main__':
    sys.path.insert(0, HERE)
    try:
        for m in sys.argv[1:]:
            emit(importlib.import_module(m))
    finally:
        shutil.rmtree(WORK, ignore_errors=True)
