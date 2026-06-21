use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(name = "dll-deps", version, about = "Recursive Windows DLL dependency walker")]
struct Cli {
    file: PathBuf,
    #[arg(long)]
    flat: bool,
    #[arg(long)]
    missing_only: bool,
    #[arg(long)]
    json: bool,
    #[arg(long, default_value_t = 8)]
    depth: usize,
}

fn main() -> ExitCode {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info.payload().downcast_ref::<String>().map(|s| s.as_str())
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .unwrap_or("");
        if msg.contains("failed printing to stdout") {
            std::process::exit(0);
        }
        default_hook(info);
    }));

    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Node {
    name: String,
    resolved: Option<PathBuf>,
    children: Vec<Node>,
}

fn run(cli: Cli) -> Result<(), String> {
    let root_path = cli.file.canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", cli.file.display()))?;
    let mut cache: HashMap<String, Option<PathBuf>> = HashMap::new();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let root_name = root_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| cli.file.display().to_string());
    let app_dir = root_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let search_dirs = build_search_dirs(&app_dir);

    let root = walk(
        &root_name,
        Some(root_path.clone()),
        cli.depth,
        &search_dirs,
        &mut cache,
        &mut visited,
    )?;

    if cli.json {
        let v = node_to_json(&root);
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
        return Ok(());
    }

    if cli.missing_only {
        for name in collect_missing(&root) {
            println!("{name}");
        }
        return Ok(());
    }

    if cli.flat {
        let mut all: BTreeSet<(String, Option<PathBuf>)> = BTreeSet::new();
        collect_flat(&root, &mut all);
        for (name, path) in all {
            match path {
                Some(p) => println!("{name}  [{}]", p.display()),
                None => println!("{name}  [MISSING]"),
            }
        }
        return Ok(());
    }

    print_tree(&root, "", true, true);
    Ok(())
}

fn build_search_dirs(app_dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![app_dir.to_path_buf()];
    let win = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    out.push(win.join("System32"));
    out.push(win.join("SysWOW64"));
    out.push(win.clone());
    if let Ok(cwd) = std::env::current_dir() {
        if !out.iter().any(|p| p == &cwd) {
            out.push(cwd);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&path_var) {
            if !out.iter().any(|d| d == &p) {
                out.push(p);
            }
        }
    }
    out
}

fn resolve(
    name: &str,
    search: &[PathBuf],
    cache: &mut HashMap<String, Option<PathBuf>>,
) -> Option<PathBuf> {
    if let Some(hit) = cache.get(&name.to_ascii_lowercase()) {
        return hit.clone();
    }
    let resolved = search.iter().find_map(|d| {
        let p = d.join(name);
        p.is_file().then_some(p)
    });
    cache.insert(name.to_ascii_lowercase(), resolved.clone());
    resolved
}

fn walk(
    name: &str,
    path: Option<PathBuf>,
    depth_remaining: usize,
    search: &[PathBuf],
    cache: &mut HashMap<String, Option<PathBuf>>,
    visited: &mut BTreeSet<String>,
) -> Result<Node, String> {
    let key = name.to_ascii_lowercase();
    if visited.contains(&key) || depth_remaining == 0 {
        return Ok(Node {
            name: name.to_string(),
            resolved: path,
            children: Vec::new(),
        });
    }
    visited.insert(key);

    let mut children = Vec::new();
    if let Some(ref p) = path {
        match fs::read(p) {
            Ok(bytes) => {
                if let Ok(deps) = parse_imports(&bytes) {
                    for dep in deps {
                        let dep_path = resolve(&dep, search, cache);
                        let child = walk(
                            &dep,
                            dep_path,
                            depth_remaining - 1,
                            search,
                            cache,
                            visited,
                        )?;
                        children.push(child);
                    }
                }
            }
            Err(_) => {}
        }
    }
    Ok(Node {
        name: name.to_string(),
        resolved: path,
        children,
    })
}

fn print_tree(node: &Node, prefix: &str, is_last: bool, is_root: bool) {
    let connector = if is_root {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };
    let label = match &node.resolved {
        Some(p) => format!("{} [{}]", node.name, p.display()),
        None => format!("{} [MISSING]", node.name),
    };
    println!("{prefix}{connector}{label}");
    let new_prefix = if is_root {
        String::new()
    } else if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}│   ")
    };
    for (i, child) in node.children.iter().enumerate() {
        print_tree(child, &new_prefix, i + 1 == node.children.len(), false);
    }
}

fn collect_flat(node: &Node, out: &mut BTreeSet<(String, Option<PathBuf>)>) {
    out.insert((node.name.clone(), node.resolved.clone()));
    for c in &node.children {
        collect_flat(c, out);
    }
}

fn collect_missing(node: &Node) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect_missing_rec(node, &mut out);
    out
}

fn collect_missing_rec(node: &Node, out: &mut BTreeSet<String>) {
    if node.resolved.is_none() {
        out.insert(node.name.clone());
    }
    for c in &node.children {
        collect_missing_rec(c, out);
    }
}

fn node_to_json(node: &Node) -> serde_json::Value {
    serde_json::json!({
        "name": node.name,
        "resolved": node.resolved.as_ref().map(|p| p.display().to_string()),
        "children": node.children.iter().map(node_to_json).collect::<Vec<_>>(),
    })
}

fn parse_imports(bytes: &[u8]) -> Result<Vec<String>, String> {
    if bytes.len() < 0x40 {
        return Err("too small".into());
    }
    if u16::from_le_bytes([bytes[0], bytes[1]]) != 0x5a4d {
        return Err("not a PE".into());
    }
    let e_lfanew = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    if e_lfanew + 24 > bytes.len() {
        return Err("nt headers truncated".into());
    }
    if u32::from_le_bytes(bytes[e_lfanew..e_lfanew + 4].try_into().unwrap()) != 0x4550 {
        return Err("missing PE signature".into());
    }
    let oh_off = e_lfanew + 24;
    let magic = u16::from_le_bytes(bytes[oh_off..oh_off + 2].try_into().unwrap());
    let is_64 = match magic {
        0x10b => false,
        0x20b => true,
        _ => return Err("bad optional header magic".into()),
    };
    let num_sections =
        u16::from_le_bytes(bytes[e_lfanew + 6..e_lfanew + 8].try_into().unwrap()) as usize;
    let size_of_opt_hdr =
        u16::from_le_bytes(bytes[e_lfanew + 20..e_lfanew + 22].try_into().unwrap()) as usize;

    let data_dir_off = if is_64 { oh_off + 112 } else { oh_off + 96 };
    if data_dir_off + 16 > bytes.len() {
        return Err("data directory truncated".into());
    }
    let import_rva =
        u32::from_le_bytes(bytes[data_dir_off + 8..data_dir_off + 12].try_into().unwrap());
    if import_rva == 0 {
        return Ok(Vec::new());
    }

    let sections_off = oh_off + size_of_opt_hdr;
    if sections_off + num_sections * 40 > bytes.len() {
        return Err("sections truncated".into());
    }
    let rva_to_file = |rva: u32| -> Option<usize> {
        for i in 0..num_sections {
            let so = sections_off + i * 40;
            let vsize = u32::from_le_bytes(bytes[so + 8..so + 12].try_into().unwrap());
            let vaddr = u32::from_le_bytes(bytes[so + 12..so + 16].try_into().unwrap());
            let rsize = u32::from_le_bytes(bytes[so + 16..so + 20].try_into().unwrap());
            let raddr = u32::from_le_bytes(bytes[so + 20..so + 24].try_into().unwrap());
            let span = vsize.max(rsize);
            if rva >= vaddr && rva < vaddr.saturating_add(span) {
                return Some(raddr as usize + (rva - vaddr) as usize);
            }
        }
        None
    };

    let mut out = Vec::new();
    let mut pos = rva_to_file(import_rva).ok_or("import directory RVA bad")?;
    loop {
        if pos + 20 > bytes.len() {
            break;
        }
        let oft = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        let name_rva = u32::from_le_bytes(bytes[pos + 12..pos + 16].try_into().unwrap());
        let first_thunk = u32::from_le_bytes(bytes[pos + 16..pos + 20].try_into().unwrap());
        if name_rva == 0 && first_thunk == 0 && oft == 0 {
            break;
        }
        pos += 20;
        if let Some(off) = rva_to_file(name_rva) {
            let end = bytes[off..].iter().position(|&b| b == 0).unwrap_or(0);
            if end > 0 {
                out.push(String::from_utf8_lossy(&bytes[off..off + end]).into_owned());
            }
        }
    }
    Ok(out)
}
