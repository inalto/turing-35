import re, os, sys, io
src = sys.argv[1]; dst = sys.argv[2]
BS = chr(92)
ESC = {'n': '\n', 't': '\t', 'r': '\r', '0': '\0', BS: BS, '"': '"', "'": "'", 'a': '\a', 'b': '\b', 'f': '\f', 'v': '\v'}

def unescape(lit):
    out = []; i = 0
    while i < len(lit):
        c = lit[i]
        if c == BS:
            n = lit[i+1]
            if n == 'u':
                out.append(chr(int(lit[i+2:i+6], 16))); i += 6; continue
            if n == 'x':
                m = re.match(r'[0-9a-fA-F]{1,4}', lit[i+2:]); out.append(chr(int(m.group(0), 16))); i += 2 + len(m.group(0)); continue
            out.append(ESC[n]); i += 2; continue
        out.append(c); i += 1
    return ''.join(out)

def decode(s, key):
    num = (2055530166 + key + 39 + 48) & 0xffffffff
    res = []
    for ch in s:
        c = ord(ch)
        b = ((c & 0xFF) ^ num) & 0xFF; num = (num + 1) & 0xffffffff
        b2 = ((c >> 8) ^ num) & 0xFF; num = (num + 1) & 0xffffffff
        res.append(chr((b << 8) | b2))
    return ''.join(res)

def cs_escape(s):
    o = []
    for ch in s:
        if ch == '"': o.append(BS + '"')
        elif ch == BS: o.append(BS + BS)
        elif ch == '\n': o.append(BS + 'n')
        elif ch == '\r': o.append(BS + 'r')
        elif ch == '\t': o.append(BS + 't')
        elif ord(ch) < 32 or (ord(ch) > 126 and not ch.isprintable()): o.append(BS + 'u%04x' % ord(ch))
        else: o.append(ch)
    return ''.join(o)

call = re.compile(r'r\.b\("((?:[^"\\]|\\.)*)",\s*(a_|-?\d+|[A-Za-z_][A-Za-z_0-9]*)\)')
total = 0; fails = 0
for root, _, files in os.walk(src):
    for f in files:
        if not f.endswith('.cs'): continue
        p = os.path.join(root, f); rel = os.path.relpath(p, src)
        text = io.open(p, encoding='utf-8').read()
        lines = text.split('\n'); key = None; out = []
        for line in lines:
            m = re.search(r'\bint a_ = (-?\d+);', line)
            if m: key = int(m.group(1))
            def rep(mm):
                global total, fails
                k = mm.group(2)
                kk = int(k) if k.lstrip('-').isdigit() else (key if k == 'a_' else None)
                if kk is None:
                    fails += 1; return mm.group(0)
                total += 1
                return '/*dec*/"' + cs_escape(decode(unescape(mm.group(1)), kk)) + '"'
            out.append(call.sub(rep, line))
        op = os.path.join(dst, rel); os.makedirs(os.path.dirname(op), exist_ok=True)
        io.open(op, 'w', encoding='utf-8').write('\n'.join(out))
print('decoded', total, 'unresolved', fails)
