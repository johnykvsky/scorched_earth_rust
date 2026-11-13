# List code that reads/writes given DS variables in SCORCH.EXE.
# Usage: refs.py SCORCH.EXE 515a 50fa ...
# Finds `mov ax,[var]` / `mov [var],ax` and any ModRM [disp16] form (incl. FPU-emulator ops).
import sys
b = open(sys.argv[1], 'rb').read(); code = b[0x6a00:0x55d80]
for var in (int(v, 16) for v in sys.argv[2:]):
    w = var.to_bytes(2, 'little'); p = code.find(w)
    while p != -1:
        prev = code[p-1]
        if prev in (0xa1, 0xa3) or (prev & 0xC7) == 0x06:
            lin = p - 2 if (prev & 0xC7) == 0x06 else p - 1
            print(f'DS:{var:04x}  linear {lin:#x}  seg {0x1000 + lin//16:04x}:{lin%16:04x}  bytes {code[lin-1:p+4].hex()}')
        p = code.find(w, p + 1)
