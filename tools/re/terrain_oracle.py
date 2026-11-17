#!/usr/bin/env python3
"""Run v1.5's integer terrain routine in a deliberately small 16-bit interpreter.

Usage: python3 tools/re/terrain_oracle.py /path/to/SCORCH.EXE > fixture.toml
No game assets are distributed. Requires Python 3 and objdump (via dis.py).
Only RNG and video driver calls are stubbed; generation, clipping and control
flow execute the original instructions. The injected RNG is our xorshift64, so
the result compares algorithms without depending on DOS clock/RNG state.
Unsupported instructions/calls fail closed rather than being approximated.
"""
import hashlib
import pathlib
import re
import subprocess
import struct
import sys

SHA256 = "05e4a11643ec7055b1c811531a73c5b4f5c67c463612f3e4910e557301c3179f"


def signed(value):
    return value - 65536 if value >= 32768 else value


class Random:
    def __init__(self, seed):
        self.state = max(1, seed)

    def range(self, maximum):
        self.state ^= (self.state << 13) & ((1 << 64) - 1)
        self.state ^= self.state >> 7
        self.state ^= (self.state << 17) & ((1 << 64) - 1)
        return self.state % max(1, maximum)


class CPU:
    def __init__(self, code, rng):
        self.code, self.rng = code, rng
        self.r = dict.fromkeys(["ax", "bx", "cx", "dx", "si", "di", "bp"], 0)
        self.r.update(sp=0xf000, ss=0x3000, ds=0x4000, es=0, cs=0x223a)
        self.memory, self.surface = {}, {}
        self.flags = (0, 0)
        self.store(0x4000, 0x519a, 0xffff)  # sufficient stack; skip overflow handler

    def load(self, segment, offset):
        return self.memory.get((segment, offset & 0xffff), 0)

    def store(self, segment, offset, value):
        self.memory[segment, offset & 0xffff] = value & 0xffff

    def address(self, operand):
        operand = operand.replace("DWORD PTR ", "").replace("WORD PTR ", "")
        parts = operand.split(":", 1)
        segment = parts[0] if len(parts) == 2 else None
        expression = parts[-1].strip("[]")
        value = sum((1 if sign != "-" else -1) *
                    (self.r[term] if term in self.r else int(term, 0))
                    for sign, term in re.findall(r"([+-]?)([a-z]+|0x[0-9a-f]+)", expression))
        segment = segment or ("ss" if "bp" in expression else "ds")
        return self.r[segment], value & 0xffff

    def read(self, operand):
        if operand in self.r:
            return self.r[operand]
        if "PTR" in operand or "[" in operand or operand.startswith(("ds:", "es:", "ss:")):
            return self.load(*self.address(operand))
        return int(operand, 0) & 0xffff

    def write(self, operand, value):
        if operand in self.r:
            self.r[operand] = value & 0xffff
        else:
            self.store(*self.address(operand), value)

    def push(self, value):
        self.r["sp"] = (self.r["sp"] - 2) & 0xffff
        self.store(self.r["ss"], self.r["sp"], value)

    def pop(self):
        value = self.load(self.r["ss"], self.r["sp"])
        self.r["sp"] = (self.r["sp"] + 2) & 0xffff
        return value

    def generate(self, width, top, bottom, bump=20, slope=20, flat=False, reverse=False, ceiling=False):
        self.surface = {}
        for offset, value in {2: int(ceiling), 4: bottom, 6: top, 8: -1 if reverse else 1,
                              10: bump, 12: slope, 14: int(flat)}.items():
            self.store(self.r["ds"], 0x1000 + offset, value)
        # cdecl far call: object far pointer, xmin, ymax, xmax, ymin.
        for value in [top, width - 1, bottom, 0, self.r["ds"], 0x1000, self.r["cs"], 0xffff]:
            self.push(value)
        ip = 0xa68
        for _ in range(2_000_000):
            if ip == 0xffff:
                assert len(self.surface) == width
                self.r["sp"] = (self.r["sp"] + 12) & 0xffff
                return [self.surface[x] for x in range(width)]
            following, op, args = self.code[ip]
            ip = following
            a = [item.strip() for item in args.split(",")]
            if op == "enter":
                assert a[1] == "0x0"
                self.push(self.r["bp"])
                self.r["bp"] = self.r["sp"]
                self.r["sp"] -= int(a[0], 0)
            elif op == "leave":
                self.r["sp"] = self.r["bp"]
                self.r["bp"] = self.pop()
            elif op == "push": self.push(self.read(a[0]))
            elif op == "pop": self.write(a[0], self.pop())
            elif op == "mov": self.write(a[0], self.read(a[1]))
            elif op == "les":
                seg, off = self.address(a[1])
                self.write(a[0], self.load(seg, off))
                self.r["es"] = self.load(seg, off + 2)
            elif op == "lea": self.write(a[0], self.address(a[1])[1])
            elif op in ("cmp", "test", "add", "sub", "xor", "or"):
                left, right = self.read(a[0]), self.read(a[1])
                if op == "cmp": self.flags = (left, right)
                elif op == "test": self.flags = (left & right, 0)
                else:
                    value = {"add": lambda: left + right, "sub": lambda: left - right,
                             "xor": lambda: left ^ right, "or": lambda: left | right}[op]()
                    self.write(a[0], value)
                    self.flags = (value & 0xffff, 0)
            elif op in ("inc", "dec"):
                self.write(a[0], self.read(a[0]) + (1 if op == "inc" else -1))
            elif op == "imul":
                assert len(a) == 1
                value = signed(self.r["ax"]) * signed(self.read(a[0]))
                self.r["ax"], self.r["dx"] = value & 0xffff, (value >> 16) & 0xffff
            elif op == "call":
                if a[0] == "0x2bf9:0x48b":
                    maximum = signed(self.load(self.r["ss"], self.r["sp"]))
                    assert maximum > 0
                    self.r["ax"] = self.rng.range(maximum)
                elif a[0] == "DWORD PTR ds:0xef10":
                    x, y0, y1, color = [self.load(self.r["ss"], self.r["sp"] + i * 2) for i in range(4)]
                    if color == 0x50: self.surface[x] = y1 + 1 if ceiling else y0
                elif a[0] == "0x980":
                    self.push(ip)
                    ip = 0x980
                else: raise ValueError(f"Unsupported call {a[0]}")
            elif op == "retf":
                ip, self.r["cs"] = self.pop(), self.pop()
            elif op.startswith("j"):
                left, right = self.flags
                ls, rs = signed(left), signed(right)
                take = {"jmp": True, "ja": left > right, "je": left == right,
                        "jne": left != right, "jl": ls < rs, "jle": ls <= rs,
                        "jg": ls > rs, "jge": ls >= rs}[op]
                if take: ip = int(a[0], 0)
            elif op != "nop": raise ValueError(f"Unsupported instruction {op} {args}")
        raise RuntimeError("Instruction budget exhausted")


def decode(path):
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise ValueError("Expected the documented v1.5 shareware SCORCH.EXE")
    # Verify constructor selects this method and hard-codes the ignored sliders.
    assert data[0x55d80+0x508c:0x55d80+0x5090] == bytes.fromhex("68 0a 3a 22")
    relocations, header_paragraphs = struct.unpack_from("<HH", data, 6)
    relocation_table = struct.unpack_from("<H", data, 24)[0]
    targets = {}
    for index in range(relocations):
        offset, segment = struct.unpack_from("<HH", data, relocation_table + 4 * index)
        at = 16 * header_paragraphs + 16 * segment + offset
        target_offset, target_segment = struct.unpack_from("<HH", data, at - 2)
        targets.setdefault(target_segment * 16 + target_offset, []).append(at - 2)
    assert 0x55d80 + 0x508c in targets[0x223a0 + 0xa68]
    assert not targets.get(0x1d900 + 0x2e), "Unexpected relocated pointer to midpoint wrapper"
    constructor = data[0x6a00+0x223a0+0xb6b:0x6a00+0x223a0+0xbcb]
    for instruction in ["26 c7 07 8c 50", "26 c7 47 0a 14 00", "26 c7 47 0c 14 00", "26 c7 47 0e 00 00"]:
        assert bytes.fromhex(instruction) in constructor
    output = subprocess.check_output([sys.executable, str(pathlib.Path(__file__).with_name("dis.py")),
                                      str(path), "323a:0980", "0x1eb"], text=True)
    rows = []
    for line in output.splitlines():
        location, asm = line.split("\t", 1)
        ip = int(location.split(":")[1].split()[0], 16)
        op, _, args = asm.strip().partition(" ")
        rows.append((ip, op, args.strip()))
    return {ip: (rows[i+1][0] if i+1 < len(rows) else 0xb6b, op, args)
            for i, (ip, op, args) in enumerate(rows)}


def main():
    code = decode(pathlib.Path(sys.argv[1]))
    print(f'# Generated by tools/re/terrain_oracle.py; original integer instructions, injected RNG.\nexe_sha256 = "{SHA256}"')
    cases = [(1, 640, 22, 479, 20, 20, False, False, False),
             (7919, 640, 22, 479, 20, 20, False, True, False),
             (98431, 640, 22, 479, 20, 20, False, False, True),
             (727, 640, 120, 479, 20, 20, False, True, True),
             (7, 64, 22, 479, 0, 20, False, False, False),
             (88, 64, 22, 479, 100, 100, True, False, False),
             (191, 64, 320, 479, 100, 100, False, True, True)]
    for seed, width, top, bottom, bump, slope, flat, reverse, cavern in cases:
        cpu = CPU(code, Random(seed))
        split = (top + bottom) // 2
        floor = cpu.generate(width, split if cavern else top, bottom, bump, slope, flat, reverse)
        ceiling = cpu.generate(width, top, split, bump, slope, flat, reverse, True) if cavern else []
        print('\n[[case]]')
        for name, value in dict(seed=seed, width=width, top=top, bottom=bottom, bump=bump,
                                slope=slope, flat=flat, reverse=reverse, cavern=cavern,
                                floor=floor, ceiling=ceiling).items():
            print(f'{name} = {str(value).lower()}')


if __name__ == "__main__":
    main()
