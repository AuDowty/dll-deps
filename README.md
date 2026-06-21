# dll-deps

Recursive Windows DLL dependency walker. Replaces the long-abandoned [Dependency Walker](https://www.dependencywalker.com/).

Pure file inspection — never loads the DLL. Cross-platform: runs on any OS (parses PE imports), but resolves to local disk only on Windows.

## Install

```
cargo install --git https://github.com/AuDowty/dll-deps
```

## Use

```
dll-deps foo.exe                    # tree view
dll-deps foo.exe --flat             # one DLL per line, deduped
dll-deps foo.exe --missing-only     # only show DLLs we couldn't resolve
dll-deps foo.exe --json             # machine-readable
dll-deps foo.exe --depth 3          # limit recursion
```

Output (tree):

```
foo.exe
├── KERNEL32.dll [C:\Windows\System32\KERNEL32.DLL]
│   ├── ntdll.dll [C:\Windows\System32\ntdll.dll]
│   └── KERNELBASE.dll [C:\Windows\System32\KERNELBASE.dll]
├── VCRUNTIME140.dll [C:\Windows\System32\VCRUNTIME140.dll]
└── api-ms-win-crt-runtime-l1-1-0.dll [C:\Windows\System32\api-ms-win-crt-runtime-l1-1-0.dll]
```

Resolution order mirrors the Windows loader: application directory, `%SystemRoot%\System32`, `%SystemRoot%\SysWOW64` (for 32-bit), `%SystemRoot%`, current directory, `%PATH%`. Missing DLLs are flagged.

## License

MIT.
