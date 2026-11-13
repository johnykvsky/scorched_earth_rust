# List far/near callers of a routine in SCORCH.EXE.  Usage: callers.py SCORCH.EXE SEG:OFF
import sys
b = open(sys.argv[1], 'rb').read(); code = b[0x6a00:0x55d80]
seg, off = (int(x, 16) for x in sys.argv[2].split(':'))
f = (seg - 0x1000) * 16 + off
def where(L): return '%04x:%04x' % (0x1000 + L // 16, L % 16)
for p in range(len(code) - 5):
    if code[p] == 0x9a:
        o = int.from_bytes(code[p+1:p+3], 'little'); s = int.from_bytes(code[p+3:p+5], 'little')
        if s * 16 + o == f: print('far ', where(p))
    elif code[p] == 0xe8 and code[p-1] == 0x0e:
        rel = int.from_bytes(code[p+1:p+3], 'little', signed=True)
        if p + 3 + rel == f: print('near', where(p))
