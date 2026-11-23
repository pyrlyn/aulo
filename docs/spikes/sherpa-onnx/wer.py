"""WER of a hypothesis TSV (file<TAB>text) against a FLEURS dev.tsv, with a plain normalizer.

Usage, from the directory that holds both files: wer.py <dev.tsv> <hypotheses.tsv> [N worst utterances to print]
"""
import re
import sys
import unicodedata
from pathlib import Path


def norm(s: str) -> list[str]:
    s = unicodedata.normalize("NFKC", s).lower().replace("ё", "е")
    s = re.sub(r"[^\w\s']|_", " ", s)
    s = s.replace("'", "")
    return s.split()


def edits(r: list[str], h: list[str]) -> int:
    prev = list(range(len(h) + 1))
    for i, rw in enumerate(r, 1):
        cur = [i] + [0] * len(h)
        for j, hw in enumerate(h, 1):
            cur[j] = min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (rw != hw))
        prev = cur
    return prev[-1]


def read_lines(name: str) -> list[str]:
    # The script only scores files it was pointed at in the working directory, so a path that
    # resolves elsewhere (`..`, an absolute path, a symlink out) is refused rather than read.
    base = Path.cwd().resolve()
    path = (base / name).resolve()
    if not path.is_relative_to(base) or not path.is_file():
        sys.exit(f"not a file in {base}: {name}")
    with path.open(encoding="utf-8") as f:
        return f.read().splitlines()


def main(ref_tsv: str, hyp_tsv: str, worst: int) -> None:
    refs = {}
    for line in read_lines(ref_tsv):
        cols = line.split("\t")
        refs[cols[1]] = cols[2]  # raw transcription, with digits as written
    errs = words = 0
    digit_utts = 0
    nd_errs = nd_words = 0
    per = []
    for line in read_lines(hyp_tsv):
        name, _, hyp = line.partition("\t")
        ref = refs[name]
        r, h = norm(ref), norm(hyp)
        e = edits(r, h)
        errs += e
        words += len(r)
        per.append((e / max(1, len(r)), name, ref, hyp))
        if re.search(r"\d", ref + hyp):
            digit_utts += 1
        else:
            nd_errs += e
            nd_words += len(r)
    print(f"utterances={len(per)} words={words} errors={errs} WER={100 * errs / words:.2f}% with_digits={digit_utts} WER_without_digit_utts={100 * nd_errs / nd_words:.2f}%")
    per.sort(reverse=True)
    for w, name, ref, hyp in per[:worst]:
        print(f"{w:.2f} {name}\n  REF {ref}\n  HYP {hyp}")


main(sys.argv[1], sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 0)
