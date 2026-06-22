# dll-deps

Recursive Windows DLL dependency walker. Replacement for the abandoned [Dependency Walker](https://www.dependencywalker.com/).

Pure file inspection — never loads the DLL. Cross-platform (parses PE imports on any OS), but resolves paths only on Windows.

## Install

```
cargo install --git https://github.com/AuDowty/dll-deps
```

## Use

```
dll-deps foo.exe                    # tree view
dll-deps foo.exe --flat             # one DLL per line, deduped
dll-deps foo.exe --missing-only     # only unresolved DLLs
dll-deps foo.exe --json             # machine-readable
dll-deps foo.exe --depth 3          # limit recursion
```

Resolution order mirrors the Windows loader: application dir, System32, SysWOW64, Windows dir, CWD, PATH.

## License

MIT
