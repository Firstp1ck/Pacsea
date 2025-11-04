Here’s a focused, incremental plan to add scanelf and checksec into Pacsea’s scan pipeline.

1) Decide where to run them
- Add both tools to:
  - build_scan_cmds_for_pkg: after Semgrep (they’ll only report if binaries are present in the repo or src/)
  - build_scan_cmds_in_dir: after Semgrep (best impact when run on a built package directory with ELF files)

2) Add the scan steps
- Insert a new section header and commands (both functions):

```/dev/null/scan_snippets.sh#L1-15
echo '--- Binary hardening checks (scanelf/checksec) ---'
# scanelf (RPATH/RUNPATH + NEEDED)
(command -v scanelf >/dev/null 2>&1 && {
  scanelf -R -q -n . | tee ./.pacsea_scan_scanelf_needed.txt
  scanelf -R -q -r . | tee ./.pacsea_scan_scanelf_rpath.txt
} || echo 'scanelf (pax-utils) not found; skipping')

# checksec (dir-wide)
(command -v checksec >/dev/null 2>&1 && {
  checksec --dir . | tee ./.pacsea_scan_checksec.txt
} || echo 'checksec not found; skipping')
```

Optional: only run if ELF files exist (to reduce noise when only sources are present):
```/dev/null/scan_snippets.sh#L17-30
has_elf() { find . -type f -exec sh -c 'file -b "$1" | grep -q "^ELF"' _ {} \; -print -quit | grep -q .; }
echo '--- Binary hardening checks (scanelf/checksec) ---'
if has_elf; then
  (command -v scanelf >/dev/null 2>&1 && {
    scanelf -R -q -n . | tee ./.pacsea_scan_scanelf_needed.txt
    scanelf -R -q -r . | tee ./.pacsea_scan_scanelf_rpath.txt
  } || echo 'scanelf (pax-utils) not found; skipping')
  (command -v checksec >/dev/null 2>&1 && {
    checksec --dir . | tee ./.pacsea_scan_checksec.txt
  } || echo 'checksec not found; skipping')
else
  echo 'No ELF files detected; skipping scanelf/checksec'
fi
```

3) Extend the summary
- Append to the existing summary block (both functions):

```/dev/null/scan_snippets.sh#L32-55
# scanelf summary (RPATH/RUNPATH)
if [ -f ./.pacsea_scan_scanelf_rpath.txt ]; then
  r=$(grep -vc '^$' ./.pacsea_scan_scanelf_rpath.txt)
  if [ "$r" -gt 0 ]; then
    echo "scanelf: files with RPATH/RUNPATH entries: $r"
  else
    echo "scanelf: no RPATH/RUNPATH entries"
  fi
else
  echo 'scanelf: not run'
fi

# checksec summary (grep common weaknesses)
if [ -f ./.pacsea_scan_checksec.txt ]; then
  relro=$(grep -cE '\bNo RELRO\b' ./.pacsea_scan_checksec.txt || true)
  can=$(grep -cE '\bNo canary found\b' ./.pacsea_scan_checksec.txt || true)
  nx=$(grep -cE '\bNX disabled\b' ./.pacsea_scan_checksec.txt || true)
  pie=$(grep -cE '\bNo PIE\b|\bPIE disabled\b' ./.pacsea_scan_checksec.txt || true)
  echo "checksec: no_relro=$relro no_canary=$can nx_disabled=$nx pie_disabled=$pie"
else
  echo 'checksec: not run'
fi
```

4) Update tests
- In src/install/scan.rs test module, assert the new header appears in both command builders:
  - joined.contains("--- Binary hardening checks (scanelf/checksec) ---")

5) Validate locally
- Ensure tools are installed: pacman -S pax-utils checksec
- Run in-place scan on a directory that contains built binaries to see meaningful results.
- Confirm artifacts are created: .pacsea_scan_scanelf_* and .pacsea_scan_checksec.txt, and summary shows counts.

6) Consider UX tweaks
- If you often scan only sources (no binaries), keep the ELF detection guard to avoid clutter.
- If you want stronger signal, optionally add a “build-then-scan” mode later (clean chroot via extra-x86_64-build) and run scanelf/checksec on the build outputs.

7) Document and ship
- Add a short README section noting the new checks and how to interpret the summary.
- Run:
  - cargo check
  - cargo fmt
  - cargo clippy
