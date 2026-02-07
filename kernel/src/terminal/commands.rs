//! Linux-Compatible Command Implementations
//!
//! 50+ commands organized by category: filesystem, text processing,
//! system info, utilities, network (stubs), and misc.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use alloc::format;

use super::fs::{self, InodeContent, FileMode};
use super::shell;

// ────────────────────────── Result type ──────────────────────────

pub struct CmdResult {
    pub output: Vec<String>,
    pub success: bool,
}

impl CmdResult {
    pub fn ok(output: Vec<String>) -> Self { Self { output, success: true } }
    pub fn ok_one(line: String) -> Self { Self { output: vec![line], success: true } }
    pub fn ok_empty() -> Self { Self { output: Vec::new(), success: true } }
    pub fn err(msg: String) -> Self { Self { output: vec![msg], success: false } }
}

// ────────────────────────── Command registry ──────────────────────────

/// All available command names (sorted).
pub static COMMAND_LIST: &[&str] = &[
    "acpi", "alias", "base64", "basename", "cal", "cat", "cd", "chmod", "chown",
    "clear", "cp", "cut", "date", "df", "diff", "dirname", "du",
    "echo", "env", "exit", "export", "false", "file", "find", "free",
    "grep", "groups", "head", "help", "hexdump", "history", "hostname",
    "id", "ifconfig", "kill", "ln", "ls", "lsblk", "lspci", "man", "md5sum", "mkdir",
    "mv", "neofetch", "netstat", "nl", "od", "ping", "printenv", "printf",
    "ps", "pwd", "readlink", "realpath", "rm", "rmdir", "sed", "seq",
    "sha256sum", "sleep", "sort", "stat", "su", "tac", "tail", "tee",
    "test", "time", "touch", "top", "tr", "tree", "true", "type",
    "uname", "uniq", "unset", "uptime", "wc", "which", "who", "whoami",
    "xargs", "xxd", "yes",
];

/// Dispatch a command. `pipe_in` carries data from a previous pipe.
pub fn execute_command(name: &str, args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    match name {
        // ── Filesystem ──
        "ls"       => cmd_ls(args),
        "cd"       => cmd_cd(args),
        "pwd"      => cmd_pwd(args),
        "mkdir"    => cmd_mkdir(args),
        "rmdir"    => cmd_rmdir(args),
        "touch"    => cmd_touch(args),
        "rm"       => cmd_rm(args),
        "cp"       => cmd_cp(args),
        "mv"       => cmd_mv(args),
        "cat"      => cmd_cat(args, pipe_in),
        "head"     => cmd_head(args, pipe_in),
        "tail"     => cmd_tail(args, pipe_in),
        "find"     => cmd_find(args),
        "du"       => cmd_du(args),
        "df"       => cmd_df(args),
        "ln"       => cmd_ln(args),
        "chmod"    => cmd_chmod(args),
        "chown"    => cmd_chown(args),
        "stat"     => cmd_stat(args),
        "file"     => cmd_file(args),
        "tree"     => cmd_tree(args),
        "wc"       => cmd_wc(args, pipe_in),
        "basename" => cmd_basename(args),
        "dirname"  => cmd_dirname(args),
        "readlink" => cmd_readlink(args),
        "realpath" => cmd_realpath(args),

        // ── Text processing ──
        "echo"     => cmd_echo(args),
        "printf"   => cmd_printf(args),
        "grep"     => cmd_grep(args, pipe_in),
        "sed"      => cmd_sed(args, pipe_in),
        "sort"     => cmd_sort(args, pipe_in),
        "uniq"     => cmd_uniq(args, pipe_in),
        "cut"      => cmd_cut(args, pipe_in),
        "tr"       => cmd_tr(args, pipe_in),
        "tee"      => cmd_tee(args, pipe_in),
        "xargs"    => cmd_xargs(args, pipe_in),
        "tac"      => cmd_tac(args, pipe_in),
        "nl"       => cmd_nl(args, pipe_in),
        "od"       => cmd_od(args),

        // ── System info ──
        "uname"    => cmd_uname(args),
        "whoami"   => cmd_whoami(args),
        "hostname" => cmd_hostname(args),
        "uptime"   => cmd_uptime(args),
        "free"     => cmd_free(args),
        "top"      => cmd_top(args),
        "ps"       => cmd_ps(args),
        "kill"     => cmd_kill(args),
        "id"       => cmd_id(args),
        "groups"   => cmd_groups(args),
        "env"      => cmd_env(args),
        "printenv" => cmd_printenv(args),
        "export"   => cmd_export(args),
        "unset"    => cmd_unset(args),
        "who"      => cmd_who(args),
        "su"       => cmd_su(args),

        // ── Utilities ──
        "date"     => cmd_date(args),
        "cal"      => cmd_cal(args),
        "clear"    => cmd_clear(args),
        "history"  => cmd_history(args),
        "alias"    => cmd_alias(args),
        "which"    => cmd_which(args),
        "type"     => cmd_type(args),
        "true"     => CmdResult::ok_empty(),
        "false"    => CmdResult { output: Vec::new(), success: false },
        "yes"      => cmd_yes(args),
        "seq"      => cmd_seq(args),
        "sleep"    => cmd_sleep(args),
        "time"     => cmd_time(args),
        "test"     => cmd_test(args),
        "exit"     => cmd_exit(args),

        // ── Network (stubs) ──
        "ping"     => cmd_ping(args),
        "ifconfig" => cmd_ifconfig(args),
        "netstat"  => cmd_netstat(args),

        // ── Hardware ──
        "lspci"    => cmd_lspci(args),
        "lsblk"    => cmd_lsblk(args),
        "acpi"     => cmd_acpi(args),

        // ── Misc ──
        "man"      => cmd_man(args),
        "help"     => cmd_help(args),
        "neofetch" => cmd_neofetch(args),
        "hexdump" | "xxd" => cmd_hexdump(args),
        "base64"   => cmd_base64(args),
        "md5sum"   => cmd_md5sum(args),
        "sha256sum" => cmd_sha256sum(args),
        "diff"     => cmd_diff(args),

        _ => CmdResult::err(format!("{}: command not found", name)),
    }
}

// ════════════════════════════════════════════════════════════
//  Filesystem commands
// ════════════════════════════════════════════════════════════

fn cmd_ls(args: &[String]) -> CmdResult {
    let mut show_all = false;
    let mut long = false;
    let mut show_size = false;
    let mut paths: Vec<String> = Vec::new();

    for arg in args {
        if arg.starts_with('-') && !arg.starts_with("--") {
            for c in arg[1..].chars() {
                match c {
                    'a' => show_all = true,
                    'l' => long = true,
                    's' | 'S' => show_size = true,
                    'h' => {} // human-readable (ignored, we always show readable)
                    'R' => {} // recursive TODO
                    'F' => {} // classify
                    _ => {}
                }
            }
        } else {
            paths.push(arg.clone());
        }
    }

    if paths.is_empty() {
        paths.push(String::from("."));
    }

    let mut output = Vec::new();
    let multi = paths.len() > 1;

    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
            Some(i) => i,
            None => {
                output.push(format!("ls: cannot access '{}': No such file or directory", path));
                continue;
            }
        };

        let is_dir = fs::with_fs(|fs| fs.get(ino).map(|n| n.mode.is_dir()).unwrap_or(false));

        if !is_dir {
            // Single file
            let info = fs::with_fs(|fs| {
                let n = fs.get(ino).unwrap();
                if long || show_size {
                    format!("{} {:>4} root root {:>8} {}", n.mode.display(), n.nlink, n.size, path)
                } else {
                    path.clone()
                }
            });
            output.push(info);
            continue;
        }

        if multi { output.push(format!("{}:", path)); }

        let entries = match fs::with_fs(|fs| fs.readdir_all(ino)) {
            Some(e) => e,
            None => continue,
        };

        let mut names: Vec<(String, bool, u64, FileMode, u32)> = entries.iter()
            .filter(|(name, _)| show_all || !name.starts_with('.'))
            .map(|(name, child_ino)| {
                fs::with_fs(|fs| {
                    let node = fs.get(*child_ino).unwrap();
                    (name.clone(), node.mode.is_dir(), node.size, node.mode, node.nlink)
                })
            })
            .collect();

        names.sort_by(|a, b| a.0.cmp(&b.0));

        if long || show_size {
            for (name, is_d, size, mode, nlink) in &names {
                let suffix = if *is_d { "/" } else { "" };
                output.push(format!("{} {:>4} root root {:>8} {}{}",
                    mode.display(), nlink, size, name, suffix));
            }
        } else {
            let line: Vec<String> = names.iter().map(|(name, is_d, _, _, _)| {
                if *is_d { format!("{}/", name) } else { name.clone() }
            }).collect();
            // Show in columns if few items
            if line.len() <= 8 {
                output.push(line.join("  "));
            } else {
                for item in line { output.push(item); }
            }
        }

        if multi { output.push(String::new()); }
    }

    CmdResult::ok(output)
}

fn cmd_cd(args: &[String]) -> CmdResult {
    let target = if args.is_empty() { "~" } else { &args[0] };
    match shell::with_shell(|sh| sh.change_dir(target)) {
        Ok(()) => CmdResult::ok_empty(),
        Err(e) => CmdResult::err(e),
    }
}

fn cmd_pwd(_args: &[String]) -> CmdResult {
    CmdResult::ok_one(shell::with_shell(|sh| sh.cwd.clone()))
}

fn cmd_mkdir(args: &[String]) -> CmdResult {
    let mut parents = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg == "-p" || arg == "--parents" { parents = true; }
        else { paths.push(arg.clone()); }
    }

    if paths.is_empty() {
        return CmdResult::err(String::from("mkdir: missing operand"));
    }

    let mut output = Vec::new();
    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        if parents {
            // Create all intermediate directories
            let mut current = String::from("/");
            for component in abs.trim_start_matches('/').split('/') {
                if component.is_empty() { continue; }
                let parent_ino = fs::with_fs(|fs| fs.resolve(&current)).unwrap_or(1);
                if fs::with_fs(|fs| fs.lookup(parent_ino, component)).is_none() {
                    let _ = fs::with_fs(|fs| fs.mkdir(parent_ino, component));
                }
                if current == "/" { current.push_str(component); }
                else { current.push('/'); current.push_str(component); }
            }
        } else {
            let (parent, name) = match fs::with_fs(|fs| fs.resolve_parent(&abs)) {
                Some(p) => p,
                None => { output.push(format!("mkdir: cannot create '{}'", path)); continue; }
            };
            match fs::with_fs(|fs| fs.mkdir(parent, &name)) {
                Ok(_) => {}
                Err(e) => output.push(format!("mkdir: cannot create '{}': {}", path, e.as_str())),
            }
        }
    }

    CmdResult { output, success: true }
}

fn cmd_rmdir(args: &[String]) -> CmdResult {
    let mut output = Vec::new();
    for path in args {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let (parent, name) = match fs::with_fs(|fs| fs.resolve_parent(&abs)) {
            Some(p) => p,
            None => { output.push(format!("rmdir: failed to remove '{}': Invalid argument", path)); continue; }
        };
        match fs::with_fs(|fs| fs.remove(parent, &name)) {
            Ok(()) => {}
            Err(e) => output.push(format!("rmdir: failed to remove '{}': {}", path, e.as_str())),
        }
    }
    CmdResult { output, success: true }
}

fn cmd_touch(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("touch: missing file operand"));
    }
    let mut output = Vec::new();
    for path in args {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        // If file exists, do nothing (would update timestamp)
        if fs::with_fs(|fs| fs.resolve(&abs)).is_some() { continue; }
        let (parent, name) = match fs::with_fs(|fs| fs.resolve_parent(&abs)) {
            Some(p) => p,
            None => { output.push(format!("touch: cannot touch '{}': No such file or directory", path)); continue; }
        };
        let _ = fs::with_fs(|fs| fs.create_file(parent, &name, &[]));
    }
    CmdResult { output, success: true }
}

fn cmd_rm(args: &[String]) -> CmdResult {
    let mut recursive = false;
    let mut force = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            for c in arg[1..].chars() {
                match c {
                    'r' | 'R' => recursive = true,
                    'f' => force = true,
                    _ => {}
                }
            }
        } else {
            paths.push(arg.clone());
        }
    }

    if paths.is_empty() {
        if force { return CmdResult::ok_empty(); }
        return CmdResult::err(String::from("rm: missing operand"));
    }

    let mut output = Vec::new();
    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
            Some(i) => i,
            None => {
                if !force { output.push(format!("rm: cannot remove '{}': No such file or directory", path)); }
                continue;
            }
        };

        let is_dir = fs::with_fs(|fs| fs.get(ino).map(|n| n.mode.is_dir()).unwrap_or(false));
        if is_dir && !recursive {
            output.push(format!("rm: cannot remove '{}': Is a directory", path));
            continue;
        }

        if is_dir && recursive {
            rm_recursive(&abs, &mut output);
        } else {
            let (parent, name) = match fs::with_fs(|fs| fs.resolve_parent(&abs)) {
                Some(p) => p,
                None => continue,
            };
            if let Err(e) = fs::with_fs(|fs| fs.remove(parent, &name)) {
                output.push(format!("rm: cannot remove '{}': {}", path, e.as_str()));
            }
        }
    }

    CmdResult { output, success: true }
}

fn rm_recursive(path: &str, output: &mut Vec<String>) {
    let ino = match fs::with_fs(|fs| fs.resolve(path)) {
        Some(i) => i,
        None => return,
    };

    // First remove all children
    let children: Vec<(String, bool)> = fs::with_fs(|fs| {
        match fs.readdir(ino) {
            Some(entries) => entries.iter().map(|(name, child_ino)| {
                let is_dir = fs.get(*child_ino).map(|n| n.mode.is_dir()).unwrap_or(false);
                (name.clone(), is_dir)
            }).collect(),
            None => Vec::new(),
        }
    });

    for (name, is_dir) in children {
        let child_path = if path == "/" { format!("/{}", name) } else { format!("{}/{}", path, name) };
        if is_dir {
            rm_recursive(&child_path, output);
        } else {
            let _ = fs::with_fs(|fs| fs.remove(ino, &name));
        }
    }

    // Now remove the directory itself
    let (parent, name) = match fs::with_fs(|fs| fs.resolve_parent(path)) {
        Some(p) => p,
        None => return,
    };
    if let Err(e) = fs::with_fs(|fs| fs.remove(parent, &name)) {
        output.push(format!("rm: cannot remove '{}': {}", path, e.as_str()));
    }
}

fn cmd_cp(args: &[String]) -> CmdResult {
    let mut recursive = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg == "-r" || arg == "-R" || arg == "--recursive" { recursive = true; }
        else { paths.push(arg.clone()); }
    }

    if paths.len() < 2 {
        return CmdResult::err(String::from("cp: missing destination file operand"));
    }

    let src_path = shell::with_shell(|sh| sh.resolve_path(&paths[0]));
    let dst_path = shell::with_shell(|sh| sh.resolve_path(&paths[1]));

    let src_ino = match fs::with_fs(|fs| fs.resolve(&src_path)) {
        Some(i) => i,
        None => return CmdResult::err(format!("cp: cannot stat '{}': No such file or directory", paths[0])),
    };

    let is_dir = fs::with_fs(|fs| fs.get(src_ino).map(|n| n.mode.is_dir()).unwrap_or(false));
    if is_dir && !recursive {
        return CmdResult::err(format!("cp: -r not specified; omitting directory '{}'", paths[0]));
    }

    let (parent, name) = match fs::with_fs(|fs| fs.resolve_parent(&dst_path)) {
        Some(p) => p,
        None => return CmdResult::err(format!("cp: cannot create '{}': No such file or directory", paths[1])),
    };

    match fs::with_fs(|fs| fs.copy_file(src_ino, parent, &name)) {
        Ok(_) => CmdResult::ok_empty(),
        Err(e) => CmdResult::err(format!("cp: error: {}", e.as_str())),
    }
}

fn cmd_mv(args: &[String]) -> CmdResult {
    if args.len() < 2 {
        return CmdResult::err(String::from("mv: missing destination file operand"));
    }

    let src_path = shell::with_shell(|sh| sh.resolve_path(&args[0]));
    let dst_path = shell::with_shell(|sh| sh.resolve_path(&args[1]));

    let (src_parent, src_name) = match fs::with_fs(|fs| fs.resolve_parent(&src_path)) {
        Some(p) => p,
        None => return CmdResult::err(format!("mv: cannot stat '{}': No such file or directory", args[0])),
    };

    // Check if destination is an existing directory
    let dst_is_dir = fs::with_fs(|fs| {
        fs.resolve(&dst_path).and_then(|ino| fs.get(ino).map(|n| n.mode.is_dir())).unwrap_or(false)
    });

    let (dst_parent, dst_name) = if dst_is_dir {
        (fs::with_fs(|fs| fs.resolve(&dst_path)).unwrap(), src_name.clone())
    } else {
        match fs::with_fs(|fs| fs.resolve_parent(&dst_path)) {
            Some(p) => p,
            None => return CmdResult::err(format!("mv: cannot move to '{}': No such file or directory", args[1])),
        }
    };

    match fs::with_fs(|fs| fs.rename(src_parent, &src_name, dst_parent, &dst_name)) {
        Ok(()) => CmdResult::ok_empty(),
        Err(e) => CmdResult::err(format!("mv: error: {}", e.as_str())),
    }
}

fn cmd_cat(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut number_lines = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg == "-n" || arg == "--number" { number_lines = true; }
        else { paths.push(arg.clone()); }
    }

    // If piped input and no files, just output the piped input
    if paths.is_empty() {
        if let Some(lines) = pipe_in {
            if number_lines {
                let out: Vec<String> = lines.iter().enumerate()
                    .map(|(i, l)| format!("{:>6}\t{}", i + 1, l)).collect();
                return CmdResult::ok(out);
            }
            return CmdResult::ok(lines.to_vec());
        }
        return CmdResult::ok_empty();
    }

    let mut output = Vec::new();
    let mut line_num = 1;

    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
            Some(i) => i,
            None => { output.push(format!("cat: {}: No such file or directory", path)); continue; }
        };
        match fs::with_fs(|fs| fs.read_file(ino)) {
            Ok(data) => {
                let text = String::from_utf8_lossy(&data);
                for line in text.lines() {
                    if number_lines {
                        output.push(format!("{:>6}\t{}", line_num, line));
                        line_num += 1;
                    } else {
                        output.push(String::from(line));
                    }
                }
            }
            Err(e) => output.push(format!("cat: {}: {}", path, e.as_str())),
        }
    }

    CmdResult::ok(output)
}

fn cmd_head(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut n = 10usize;
    let mut paths = Vec::new();

    let mut i = 0;
    while i < args.len() {
        if args[i] == "-n" && i + 1 < args.len() {
            n = args[i + 1].parse().unwrap_or(10);
            i += 2;
        } else if args[i].starts_with('-') && args[i][1..].parse::<usize>().is_ok() {
            n = args[i][1..].parse().unwrap_or(10);
            i += 1;
        } else {
            paths.push(args[i].clone());
            i += 1;
        }
    }

    if paths.is_empty() {
        if let Some(lines) = pipe_in {
            return CmdResult::ok(lines.iter().take(n).cloned().collect());
        }
        return CmdResult::ok_empty();
    }

    let mut output = Vec::new();
    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let data = read_file_lines(&abs);
        match data {
            Ok(lines) => output.extend(lines.into_iter().take(n)),
            Err(e) => output.push(format!("head: {}: {}", path, e)),
        }
    }
    CmdResult::ok(output)
}

fn cmd_tail(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut n = 10usize;
    let mut paths = Vec::new();

    let mut i = 0;
    while i < args.len() {
        if args[i] == "-n" && i + 1 < args.len() {
            n = args[i + 1].parse().unwrap_or(10);
            i += 2;
        } else if args[i].starts_with('-') && args[i][1..].parse::<usize>().is_ok() {
            n = args[i][1..].parse().unwrap_or(10);
            i += 1;
        } else {
            paths.push(args[i].clone());
            i += 1;
        }
    }

    if paths.is_empty() {
        if let Some(lines) = pipe_in {
            let skip = lines.len().saturating_sub(n);
            return CmdResult::ok(lines.iter().skip(skip).cloned().collect());
        }
        return CmdResult::ok_empty();
    }

    let mut output = Vec::new();
    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let data = read_file_lines(&abs);
        match data {
            Ok(lines) => {
                let skip = lines.len().saturating_sub(n);
                output.extend(lines.into_iter().skip(skip));
            }
            Err(e) => output.push(format!("tail: {}: {}", path, e)),
        }
    }
    CmdResult::ok(output)
}

fn cmd_find(args: &[String]) -> CmdResult {
    let mut search_dir = String::from(".");
    let mut name_pattern: Option<String> = None;
    let mut type_filter: Option<char> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-name" if i + 1 < args.len() => { name_pattern = Some(args[i + 1].clone()); i += 2; }
            "-type" if i + 1 < args.len() => { type_filter = Some(args[i + 1].chars().next().unwrap_or('f')); i += 2; }
            s if !s.starts_with('-') && i == 0 => { search_dir = args[i].clone(); i += 1; }
            _ => { i += 1; }
        }
    }

    let abs = shell::with_shell(|sh| sh.resolve_path(&search_dir));
    let mut results = Vec::new();
    find_recursive(&abs, &abs, &name_pattern, type_filter, &mut results);
    CmdResult::ok(results)
}

fn find_recursive(base: &str, current: &str, pattern: &Option<String>, type_filter: Option<char>, results: &mut Vec<String>) {
    let ino = match fs::with_fs(|fs| fs.resolve(current)) {
        Some(i) => i,
        None => return,
    };

    let entries: Vec<(String, bool)> = fs::with_fs(|fs| {
        match fs.readdir(ino) {
            Some(e) => e.iter().map(|(name, child_ino)| {
                let is_dir = fs.get(*child_ino).map(|n| n.mode.is_dir()).unwrap_or(false);
                (name.clone(), is_dir)
            }).collect(),
            None => Vec::new(),
        }
    });

    for (name, is_dir) in entries {
        let child_path = if current == "/" { format!("/{}", name) } else { format!("{}/{}", current, name) };
        let display = if base == "/" {
            child_path.clone()
        } else {
            // Make relative to the search dir shown as the user typed
            let rel = &child_path[base.len()..];
            format!(".{}", rel)
        };

        let matches_type = match type_filter {
            Some('d') => is_dir,
            Some('f') => !is_dir,
            _ => true,
        };

        let matches_name = match pattern {
            Some(pat) => simple_glob_match(pat, &name),
            None => true,
        };

        if matches_type && matches_name {
            results.push(display);
        }

        if is_dir {
            find_recursive(base, &child_path, pattern, type_filter, results);
        }
    }
}

fn cmd_du(args: &[String]) -> CmdResult {
    let mut human = false;
    let mut _summary = false;
    let mut paths = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-h" | "--human-readable" => human = true,
            "-s" | "--summarize" => _summary = true,
            _ => paths.push(arg.clone()),
        }
    }

    if paths.is_empty() { paths.push(String::from(".")); }

    let mut output = Vec::new();
    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
            Some(i) => i,
            None => { output.push(format!("du: cannot access '{}': No such file or directory", path)); continue; }
        };
        let size = fs::with_fs(|fs| fs.tree_size(ino));
        let display = if human { human_size(size) } else { format!("{}", size) };
        output.push(format!("{}\t{}", display, path));
    }

    CmdResult::ok(output)
}

fn cmd_df(_args: &[String]) -> CmdResult {
    let output = vec![
        String::from("Filesystem     1K-blocks   Used Available Use% Mounted on"),
        String::from("ramfs             512000  64000    448000  13% /"),
        String::from("proc                   0      0         0   0% /proc"),
    ];
    CmdResult::ok(output)
}

fn cmd_ln(args: &[String]) -> CmdResult {
    // Simplified: only supports "ln -s target linkname"
    if args.len() < 2 {
        return CmdResult::err(String::from("ln: missing file operand"));
    }
    CmdResult::err(String::from("ln: symbolic links not yet fully supported in ramfs"))
}

fn cmd_chmod(args: &[String]) -> CmdResult {
    if args.len() < 2 {
        return CmdResult::err(String::from("chmod: missing operand"));
    }
    // Parse doesn't fully update mode, but acknowledge
    CmdResult::ok_empty()
}

fn cmd_chown(args: &[String]) -> CmdResult {
    if args.len() < 2 {
        return CmdResult::err(String::from("chown: missing operand"));
    }
    CmdResult::ok_empty()
}

fn cmd_stat(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("stat: missing operand"));
    }

    let mut output = Vec::new();
    for path in args {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
            Some(i) => i,
            None => { output.push(format!("stat: cannot stat '{}': No such file or directory", path)); continue; }
        };

        fs::with_fs(|fs| {
            if let Some(node) = fs.get(ino) {
                output.push(format!("  File: {}", path));
                output.push(format!("  Size: {}\tBlocks: {}\tIO Block: 4096",
                    node.size, (node.size + 511) / 512));
                let ftype = if node.mode.is_dir() { "directory" }
                           else if node.mode.is_symlink() { "symbolic link" }
                           else { "regular file" };
                output.push(format!("  Type: {}", ftype));
                output.push(format!("Device: ramfs\tInode: {}\tLinks: {}", node.ino, node.nlink));
                output.push(format!("Access: ({})\tUid: {}\tGid: {}",
                    node.mode.display(), node.uid, node.gid));
            }
        });
    }

    CmdResult::ok(output)
}

fn cmd_file(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("file: missing operand"));
    }

    let mut output = Vec::new();
    for path in args {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
            Some(i) => i,
            None => { output.push(format!("{}: cannot open (No such file or directory)", path)); continue; }
        };

        let desc = fs::with_fs(|fs| {
            let node = fs.get(ino).unwrap();
            match &node.content {
                InodeContent::Directory(_) => String::from("directory"),
                InodeContent::Symlink(target) => format!("symbolic link to {}", target),
                InodeContent::ProcFile(_) => String::from("proc pseudo-file"),
                InodeContent::File(data) => {
                    if data.is_empty() { String::from("empty") }
                    else if data.iter().all(|b| b.is_ascii()) { String::from("ASCII text") }
                    else { String::from("data") }
                }
            }
        });
        output.push(format!("{}: {}", path, desc));
    }

    CmdResult::ok(output)
}

fn cmd_tree(args: &[String]) -> CmdResult {
    let path = args.first().map(|s| s.as_str()).unwrap_or(".");
    let abs = shell::with_shell(|sh| sh.resolve_path(path));

    let mut lines = Vec::new();
    lines.push(String::from(path));
    let (dirs, files) = tree_recursive(&abs, &String::new(), &mut lines);
    lines.push(format!("\n{} directories, {} files", dirs, files));
    CmdResult::ok(lines)
}

fn tree_recursive(path: &str, prefix: &str, lines: &mut Vec<String>) -> (usize, usize) {
    let ino = match fs::with_fs(|fs| fs.resolve(path)) {
        Some(i) => i,
        None => return (0, 0),
    };

    let entries: Vec<(String, bool)> = fs::with_fs(|fs| {
        match fs.readdir(ino) {
            Some(e) => {
                let mut v: Vec<(String, bool)> = e.iter().map(|(name, child_ino)| {
                    let is_dir = fs.get(*child_ino).map(|n| n.mode.is_dir()).unwrap_or(false);
                    (name.clone(), is_dir)
                }).collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v
            }
            None => Vec::new(),
        }
    });

    let mut dirs = 0usize;
    let mut files = 0usize;

    for (i, (name, is_dir)) in entries.iter().enumerate() {
        let is_last = i == entries.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let display = if *is_dir { format!("{}/", name) } else { name.clone() };
        lines.push(format!("{}{}{}", prefix, connector, display));

        if *is_dir {
            dirs += 1;
            let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            let child_path = if path == "/" { format!("/{}", name) } else { format!("{}/{}", path, name) };
            let (d, f) = tree_recursive(&child_path, &child_prefix, lines);
            dirs += d;
            files += f;
        } else {
            files += 1;
        }
    }

    (dirs, files)
}

fn cmd_wc(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut paths = Vec::new();
    let mut count_lines = false;
    let mut count_words = false;
    let mut count_chars = false;
    let mut explicit = false;

    for arg in args {
        match arg.as_str() {
            "-l" => { count_lines = true; explicit = true; }
            "-w" => { count_words = true; explicit = true; }
            "-c" | "-m" => { count_chars = true; explicit = true; }
            _ => paths.push(arg.clone()),
        }
    }

    if !explicit { count_lines = true; count_words = true; count_chars = true; }

    let count_text = |text: &str| {
        let lines = text.lines().count();
        let words = text.split_whitespace().count();
        let chars = text.len();
        let mut parts = Vec::new();
        if count_lines { parts.push(format!("{:>8}", lines)); }
        if count_words { parts.push(format!("{:>8}", words)); }
        if count_chars { parts.push(format!("{:>8}", chars)); }
        parts.join("")
    };

    if paths.is_empty() {
        if let Some(lines) = pipe_in {
            let text = lines.join("\n");
            return CmdResult::ok_one(count_text(&text));
        }
        return CmdResult::ok_empty();
    }

    let mut output = Vec::new();
    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        match read_file_content(&abs) {
            Ok(text) => output.push(format!("{} {}", count_text(&text), path)),
            Err(e) => output.push(format!("wc: {}: {}", path, e)),
        }
    }
    CmdResult::ok(output)
}

fn cmd_basename(args: &[String]) -> CmdResult {
    if args.is_empty() { return CmdResult::err(String::from("basename: missing operand")); }
    let path = &args[0];
    let name = path.rsplit('/').next().unwrap_or(path);
    let result = if args.len() > 1 {
        // Strip suffix
        let suffix = &args[1];
        name.strip_suffix(suffix.as_str()).unwrap_or(name)
    } else { name };
    CmdResult::ok_one(String::from(result))
}

fn cmd_dirname(args: &[String]) -> CmdResult {
    if args.is_empty() { return CmdResult::err(String::from("dirname: missing operand")); }
    let path = &args[0];
    let dir = if let Some(idx) = path.rfind('/') {
        if idx == 0 { "/" } else { &path[..idx] }
    } else { "." };
    CmdResult::ok_one(String::from(dir))
}

fn cmd_readlink(args: &[String]) -> CmdResult {
    if args.is_empty() { return CmdResult::err(String::from("readlink: missing operand")); }
    let abs = shell::with_shell(|sh| sh.resolve_path(&args[0]));
    CmdResult::ok_one(abs)
}

fn cmd_realpath(args: &[String]) -> CmdResult {
    if args.is_empty() { return CmdResult::err(String::from("realpath: missing operand")); }
    let abs = shell::with_shell(|sh| sh.resolve_path(&args[0]));
    CmdResult::ok_one(abs)
}

// ════════════════════════════════════════════════════════════
//  Text processing commands
// ════════════════════════════════════════════════════════════

fn cmd_echo(args: &[String]) -> CmdResult {
    let mut interpret = false;
    let mut no_newline = false;
    let mut start = 0;

    for (i, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "-n" => { no_newline = true; start = i + 1; }
            "-e" => { interpret = true; start = i + 1; }
            _ => break,
        }
    }

    let text = args[start..].join(" ");
    let output = if interpret {
        text.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\")
    } else {
        text
    };

    if no_newline && output.is_empty() {
        CmdResult::ok_empty()
    } else {
        // Split by newlines in case -e produced them
        let lines: Vec<String> = output.lines().map(String::from).collect();
        if lines.is_empty() {
            CmdResult::ok_one(String::new())
        } else {
            CmdResult::ok(lines)
        }
    }
}

fn cmd_printf(args: &[String]) -> CmdResult {
    if args.is_empty() { return CmdResult::ok_empty(); }
    let fmt = &args[0];
    let text = fmt.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\");
    // Very basic %s substitution
    let mut result = String::new();
    let mut arg_idx = 1;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() && chars[i + 1] == 's' {
            if arg_idx < args.len() {
                result.push_str(&args[arg_idx]);
                arg_idx += 1;
            }
            i += 2;
        } else if chars[i] == '%' && i + 1 < chars.len() && chars[i + 1] == 'd' {
            if arg_idx < args.len() {
                result.push_str(&args[arg_idx]);
                arg_idx += 1;
            }
            i += 2;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    let lines: Vec<String> = result.lines().map(String::from).collect();
    CmdResult::ok(lines)
}

fn cmd_grep(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut invert = false;
    let mut ignore_case = false;
    let mut count_only = false;
    let mut line_number = false;
    let mut pattern: Option<String> = None;
    let mut paths = Vec::new();

    for arg in args {
        if arg.starts_with('-') && !arg.starts_with("--") {
            for c in arg[1..].chars() {
                match c {
                    'v' => invert = true,
                    'i' => ignore_case = true,
                    'c' => count_only = true,
                    'n' => line_number = true,
                    _ => {}
                }
            }
        } else if pattern.is_none() {
            pattern = Some(arg.clone());
        } else {
            paths.push(arg.clone());
        }
    }

    let pattern = match pattern {
        Some(p) => p,
        None => return CmdResult::err(String::from("grep: missing pattern")),
    };

    let match_fn = |line: &str| -> bool {
        let (hay, needle) = if ignore_case {
            (line.to_lowercase(), pattern.to_lowercase())
        } else {
            (String::from(line), pattern.clone())
        };
        let found = hay.contains(&needle);
        if invert { !found } else { found }
    };

    let lines_to_search: Vec<String> = if !paths.is_empty() {
        let mut all = Vec::new();
        for path in &paths {
            let abs = shell::with_shell(|sh| sh.resolve_path(path));
            match read_file_lines(&abs) {
                Ok(l) => all.extend(l),
                Err(e) => return CmdResult::err(format!("grep: {}: {}", path, e)),
            }
        }
        all
    } else if let Some(input) = pipe_in {
        input.to_vec()
    } else {
        return CmdResult::ok_empty();
    };

    let matched: Vec<(usize, String)> = lines_to_search.iter().enumerate()
        .filter(|(_, l)| match_fn(l))
        .map(|(i, l)| (i + 1, l.clone()))
        .collect();

    if count_only {
        return CmdResult::ok_one(format!("{}", matched.len()));
    }

    let output: Vec<String> = matched.into_iter().map(|(num, line)| {
        if line_number { format!("{}:{}", num, line) } else { line }
    }).collect();

    let success = !output.is_empty();
    CmdResult { output, success }
}

fn cmd_sed(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    // Basic s/pattern/replacement/ support
    if args.is_empty() {
        return CmdResult::err(String::from("sed: missing expression"));
    }

    let expr = &args[0];
    let paths: Vec<&String> = args[1..].iter().collect();

    let (pattern, replacement, global) = match parse_sed_expr(expr) {
        Some(p) => p,
        None => return CmdResult::err(format!("sed: invalid expression: '{}'", expr)),
    };

    let lines = if !paths.is_empty() {
        let abs = shell::with_shell(|sh| sh.resolve_path(paths[0]));
        match read_file_lines(&abs) {
            Ok(l) => l,
            Err(e) => return CmdResult::err(format!("sed: {}", e)),
        }
    } else if let Some(input) = pipe_in {
        input.to_vec()
    } else {
        return CmdResult::ok_empty();
    };

    let output: Vec<String> = lines.iter().map(|line| {
        if global {
            line.replace(&pattern, &replacement)
        } else {
            // Only first occurrence
            match line.find(&pattern) {
                Some(idx) => {
                    let mut s = String::from(&line[..idx]);
                    s.push_str(&replacement);
                    s.push_str(&line[idx + pattern.len()..]);
                    s
                }
                None => line.clone(),
            }
        }
    }).collect();

    CmdResult::ok(output)
}

fn parse_sed_expr(expr: &str) -> Option<(String, String, bool)> {
    let expr = expr.strip_prefix("s")?;
    let delim = expr.chars().next()?;
    let parts: Vec<&str> = expr[1..].split(delim).collect();
    if parts.len() < 2 { return None; }
    let global = parts.get(2).map(|s| s.contains('g')).unwrap_or(false);
    Some((String::from(parts[0]), String::from(parts[1]), global))
}

fn cmd_sort(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut reverse = false;
    let mut numeric = false;
    let mut unique = false;
    let mut paths = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-r" => reverse = true,
            "-n" => numeric = true,
            "-u" => unique = true,
            _ => paths.push(arg.clone()),
        }
    }

    let mut lines = if !paths.is_empty() {
        let abs = shell::with_shell(|sh| sh.resolve_path(&paths[0]));
        match read_file_lines(&abs) {
            Ok(l) => l,
            Err(e) => return CmdResult::err(format!("sort: {}", e)),
        }
    } else if let Some(input) = pipe_in {
        input.to_vec()
    } else {
        return CmdResult::ok_empty();
    };

    if numeric {
        lines.sort_by(|a, b| {
            let na: i64 = a.trim().split_whitespace().next()
                .and_then(|s| s.parse().ok()).unwrap_or(0);
            let nb: i64 = b.trim().split_whitespace().next()
                .and_then(|s| s.parse().ok()).unwrap_or(0);
            na.cmp(&nb)
        });
    } else {
        lines.sort();
    }

    if reverse { lines.reverse(); }
    if unique { lines.dedup(); }

    CmdResult::ok(lines)
}

fn cmd_uniq(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut count = false;
    let mut only_dup = false;
    let mut only_uniq = false;
    let mut paths = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-c" => count = true,
            "-d" => only_dup = true,
            "-u" => only_uniq = true,
            _ => paths.push(arg.clone()),
        }
    }

    let lines = if !paths.is_empty() {
        let abs = shell::with_shell(|sh| sh.resolve_path(&paths[0]));
        match read_file_lines(&abs) {
            Ok(l) => l,
            Err(e) => return CmdResult::err(format!("uniq: {}", e)),
        }
    } else if let Some(input) = pipe_in {
        input.to_vec()
    } else {
        return CmdResult::ok_empty();
    };

    // Group consecutive identical lines
    let mut groups: Vec<(usize, String)> = Vec::new();
    for line in &lines {
        if let Some(last) = groups.last_mut() {
            if &last.1 == line {
                last.0 += 1;
                continue;
            }
        }
        groups.push((1, line.clone()));
    }

    let output: Vec<String> = groups.into_iter()
        .filter(|(c, _)| {
            if only_dup { *c > 1 }
            else if only_uniq { *c == 1 }
            else { true }
        })
        .map(|(c, line)| {
            if count { format!("{:>7} {}", c, line) } else { line }
        })
        .collect();

    CmdResult::ok(output)
}

fn cmd_cut(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut delimiter = '\t';
    let mut fields: Vec<usize> = Vec::new();
    let mut paths = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" if i + 1 < args.len() => {
                delimiter = args[i + 1].chars().next().unwrap_or('\t');
                i += 2;
            }
            "-f" if i + 1 < args.len() => {
                fields = parse_field_spec(&args[i + 1]);
                i += 2;
            }
            _ => { paths.push(args[i].clone()); i += 1; }
        }
    }

    if fields.is_empty() {
        return CmdResult::err(String::from("cut: you must specify a list of fields"));
    }

    let lines = if !paths.is_empty() {
        let abs = shell::with_shell(|sh| sh.resolve_path(&paths[0]));
        match read_file_lines(&abs) {
            Ok(l) => l,
            Err(e) => return CmdResult::err(format!("cut: {}", e)),
        }
    } else if let Some(input) = pipe_in {
        input.to_vec()
    } else {
        return CmdResult::ok_empty();
    };

    let delim_str = String::from(delimiter);
    let output: Vec<String> = lines.iter().map(|line| {
        let parts: Vec<&str> = line.split(delimiter).collect();
        let selected: Vec<&str> = fields.iter()
            .filter_map(|&f| parts.get(f.saturating_sub(1)))
            .copied()
            .collect();
        selected.join(&delim_str)
    }).collect();

    CmdResult::ok(output)
}

fn parse_field_spec(spec: &str) -> Vec<usize> {
    let mut fields = Vec::new();
    for part in spec.split(',') {
        if let Some(dash) = part.find('-') {
            let start: usize = part[..dash].parse().unwrap_or(1);
            let end: usize = part[dash + 1..].parse().unwrap_or(start);
            for f in start..=end { fields.push(f); }
        } else if let Ok(f) = part.parse::<usize>() {
            fields.push(f);
        }
    }
    fields
}

fn cmd_tr(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    if args.len() < 2 {
        return CmdResult::err(String::from("tr: missing operand"));
    }

    let set1: Vec<char> = expand_tr_set(&args[0]);
    let set2: Vec<char> = expand_tr_set(&args[1]);

    let input_text = pipe_in.map(|lines| lines.join("\n")).unwrap_or_default();

    let output: String = input_text.chars().map(|c| {
        if let Some(pos) = set1.iter().position(|&s| s == c) {
            set2.get(pos).or(set2.last()).copied().unwrap_or(c)
        } else { c }
    }).collect();

    let lines: Vec<String> = output.lines().map(String::from).collect();
    CmdResult::ok(lines)
}

fn expand_tr_set(s: &str) -> Vec<char> {
    let mut chars = Vec::new();
    let input: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < input.len() {
        if i + 2 < input.len() && input[i + 1] == '-' {
            let start = input[i] as u32;
            let end = input[i + 2] as u32;
            for c in start..=end {
                if let Some(ch) = char::from_u32(c) { chars.push(ch); }
            }
            i += 3;
        } else {
            chars.push(input[i]);
            i += 1;
        }
    }
    chars
}

fn cmd_tee(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let mut append = false;
    let mut paths = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-a" => append = true,
            _ => paths.push(arg.clone()),
        }
    }

    let lines = pipe_in.unwrap_or(&[]).to_vec();
    let data = lines.join("\n");
    let data_bytes = if data.is_empty() { Vec::new() } else {
        let mut b = data.into_bytes();
        b.push(b'\n');
        b
    };

    for path in &paths {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        fs::with_fs(|fs| {
            if append {
                if let Some(ino) = fs.resolve(&abs) {
                    let _ = fs.append_file(ino, &data_bytes);
                } else if let Some((parent, name)) = fs.resolve_parent(&abs) {
                    let _ = fs.create_file(parent, &name, &data_bytes);
                }
            } else if let Some((parent, name)) = fs.resolve_parent(&abs) {
                let _ = fs.create_file(parent, &name, &data_bytes);
            }
        });
    }

    CmdResult::ok(lines)
}

fn cmd_xargs(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let cmd_name = args.first().map(|s| s.as_str()).unwrap_or("echo");
    let cmd_args: Vec<String> = args.iter().skip(1).cloned().collect();

    let input_items: Vec<String> = pipe_in.unwrap_or(&[]).iter()
        .flat_map(|line| line.split_whitespace().map(String::from).collect::<Vec<_>>())
        .collect();

    let mut all_args = cmd_args;
    all_args.extend(input_items);

    execute_command(cmd_name, &all_args, None)
}

fn cmd_tac(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let lines = if !args.is_empty() {
        let abs = shell::with_shell(|sh| sh.resolve_path(&args[0]));
        match read_file_lines(&abs) {
            Ok(l) => l,
            Err(e) => return CmdResult::err(format!("tac: {}", e)),
        }
    } else if let Some(input) = pipe_in {
        input.to_vec()
    } else {
        return CmdResult::ok_empty();
    };

    let mut reversed = lines;
    reversed.reverse();
    CmdResult::ok(reversed)
}

fn cmd_nl(args: &[String], pipe_in: Option<&[String]>) -> CmdResult {
    let lines = if !args.is_empty() && !args[0].starts_with('-') {
        let abs = shell::with_shell(|sh| sh.resolve_path(&args[0]));
        match read_file_lines(&abs) {
            Ok(l) => l,
            Err(e) => return CmdResult::err(format!("nl: {}", e)),
        }
    } else if let Some(input) = pipe_in {
        input.to_vec()
    } else {
        return CmdResult::ok_empty();
    };

    let output: Vec<String> = lines.iter().enumerate()
        .map(|(i, l)| format!("{:>6}\t{}", i + 1, l))
        .collect();
    CmdResult::ok(output)
}

fn cmd_od(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("od: missing operand"));
    }
    let abs = shell::with_shell(|sh| sh.resolve_path(&args[args.len() - 1]));
    let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
        Some(i) => i,
        None => return CmdResult::err(format!("od: {}: No such file or directory", args[args.len() - 1])),
    };
    let data = match fs::with_fs(|fs| fs.read_file(ino)) {
        Ok(d) => d,
        Err(e) => return CmdResult::err(format!("od: {}", e.as_str())),
    };

    let mut output = Vec::new();
    for (offset, chunk) in data.chunks(16).enumerate() {
        let offset_str = format!("{:07o}", offset * 16);
        let octal: Vec<String> = chunk.iter().map(|b| format!("{:03o}", b)).collect();
        output.push(format!("{} {}", offset_str, octal.join(" ")));
    }
    output.push(format!("{:07o}", data.len()));
    CmdResult::ok(output)
}

// ════════════════════════════════════════════════════════════
//  System info commands
// ════════════════════════════════════════════════════════════

fn cmd_uname(args: &[String]) -> CmdResult {
    let mut show_all = false;
    let mut show_s = true; // default: just kernel name
    let mut show_n = false;
    let mut show_r = false;
    let mut show_v = false;
    let mut show_m = false;
    let mut show_o = false;

    for arg in args {
        if arg.starts_with('-') {
            for c in arg[1..].chars() {
                match c {
                    'a' => show_all = true,
                    's' => show_s = true,
                    'n' => show_n = true,
                    'r' => show_r = true,
                    'v' => show_v = true,
                    'm' => show_m = true,
                    'o' => show_o = true,
                    _ => {}
                }
            }
        }
    }

    if show_all { show_s = true; show_n = true; show_r = true; show_v = true; show_m = true; show_o = true; }

    let mut parts = Vec::new();
    if show_s { parts.push("KPIO"); }
    if show_n { parts.push("kpio"); }
    if show_r { parts.push("1.0.0"); }
    if show_v { parts.push("#1 SMP PREEMPT_DYNAMIC"); }
    if show_m { parts.push("x86_64"); }
    if show_o { parts.push("KPIO/OS"); }

    CmdResult::ok_one(parts.join(" "))
}

fn cmd_whoami(_args: &[String]) -> CmdResult {
    let user = shell::with_shell(|sh| sh.env.get("USER").cloned().unwrap_or_else(|| String::from("root")));
    CmdResult::ok_one(user)
}

fn cmd_hostname(args: &[String]) -> CmdResult {
    if let Some(new_name) = args.first() {
        shell::with_shell(|sh| sh.env.insert(String::from("HOSTNAME"), new_name.clone()));
        CmdResult::ok_empty()
    } else {
        let host = shell::with_shell(|sh| sh.env.get("HOSTNAME").cloned().unwrap_or_else(|| String::from("kpio")));
        CmdResult::ok_one(host)
    }
}

fn cmd_uptime(_args: &[String]) -> CmdResult {
    let secs = fs::with_fs(|fs| fs.uptime_secs());
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    CmdResult::ok_one(format!(" up {}:{:02}, 1 user, load average: 0.00, 0.00, 0.00", hours, mins))
}

fn cmd_free(args: &[String]) -> CmdResult {
    let human = args.iter().any(|a| a == "-h");
    let stats = crate::allocator::heap_stats();
    let total_kb = stats.total / 1024;
    let used_kb = stats.used / 1024;
    let free_kb = stats.free / 1024;
    let output = if human {
        let hs = |kb: usize| -> String {
            if kb >= 1024 { format!("{}Mi", kb / 1024) }
            else { format!("{}Ki", kb) }
        };
        vec![
            String::from("              total        used        free      shared  buff/cache   available"),
            format!("Mem:       {:>8}    {:>8}    {:>8}        0Ki          0Ki    {:>8}",
                    hs(total_kb), hs(used_kb), hs(free_kb), hs(free_kb)),
            String::from("Swap:           0Mi          0Mi         0Mi"),
        ]
    } else {
        vec![
            String::from("              total        used        free      shared  buff/cache   available"),
            format!("Mem:       {:>8}    {:>8}    {:>8}           0           0    {:>8}",
                    total_kb, used_kb, free_kb, free_kb),
            String::from("Swap:             0           0           0"),
        ]
    };
    CmdResult::ok(output)
}

fn cmd_top(_args: &[String]) -> CmdResult {
    let secs = fs::with_fs(|fs| fs.uptime_secs());
    let stats = crate::allocator::heap_stats();
    let total_mb = stats.total / (1024 * 1024);
    let free_mb = stats.free / (1024 * 1024);
    let used_mb = stats.used / (1024 * 1024);
    let avail_mb = free_mb;
    let tasks = crate::scheduler::total_task_count();
    let ctx = crate::scheduler::context_switch_count();
    let output = vec![
        format!("top - up {}:{:02}, 1 user, load average: 0.00, 0.00, 0.00", secs / 3600, (secs % 3600) / 60),
        format!("Tasks: {:>3} total,   1 running, {:>3} sleeping,   0 stopped", tasks, tasks.saturating_sub(1)),
        String::from("%Cpu(s):  0.3 us,  0.2 sy,  0.0 ni, 99.5 id,  0.0 wa"),
        format!("MiB Mem:  {:>5}.0 total,  {:>5}.0 free,  {:>5}.0 used,    0.0 buff/cache",
                total_mb, free_mb, used_mb),
        format!("MiB Swap:    0.0 total,     0.0 free,     0.0 used, {:>5}.0 avail Mem", avail_mb),
        String::new(),
        String::from("  PID USER      PR  NI    VIRT    RES    SHR S  %CPU  %MEM     TIME+ COMMAND"),
        format!("    0 root      20   0    {:>4}    {:>4}      0 S   0.0   0.0   0:00.00 kernel", used_mb, used_mb),
        String::from("    1 root      20   0       0      0      0 S   0.0   0.0   0:00.00 idle"),
        format!("    - root      20   0       0      0      0 R   0.0   0.0   ctx:{}", ctx),
    ];
    CmdResult::ok(output)
}

fn cmd_ps(args: &[String]) -> CmdResult {
    let show_all = args.iter().any(|a| a.contains('e') || a.contains('a') || a == "aux");
    let wide = args.iter().any(|a| a.contains('f') || a == "aux");

    let mut output = Vec::new();
    if wide {
        output.push(String::from("UID        PID  PPID  C STIME TTY          TIME CMD"));
        output.push(String::from("root         1     0  0 00:00 ?        00:00:00 /sbin/init"));
        output.push(String::from("root         2     1  0 00:00 ?        00:00:00 [kpio-gui]"));
        output.push(String::from("root         3     2  0 00:00 tty0     00:00:00 /bin/sh"));
        if show_all {
            output.push(String::from("root         4     1  0 00:00 ?        00:00:00 [kworker/0:0]"));
            output.push(String::from("root         5     1  0 00:00 ?        00:00:00 [kpio-timer]"));
        }
    } else {
        output.push(String::from("  PID TTY          TIME CMD"));
        output.push(String::from("    1 ?        00:00:00 init"));
        output.push(String::from("    2 ?        00:00:00 kpio-gui"));
        output.push(String::from("    3 tty0     00:00:00 sh"));
    }

    CmdResult::ok(output)
}

fn cmd_kill(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("kill: missing operand"));
    }
    let pid = args.last().unwrap();
    CmdResult::ok_one(format!("kill: ({}) - No such process (simulated)", pid))
}

fn cmd_id(_args: &[String]) -> CmdResult {
    CmdResult::ok_one(String::from("uid=0(root) gid=0(root) groups=0(root)"))
}

fn cmd_groups(_args: &[String]) -> CmdResult {
    CmdResult::ok_one(String::from("root"))
}

fn cmd_env(_args: &[String]) -> CmdResult {
    let mut output = Vec::new();
    shell::with_shell(|sh| {
        for (k, v) in &sh.env {
            output.push(format!("{}={}", k, v));
        }
    });
    output.sort();
    CmdResult::ok(output)
}

fn cmd_printenv(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return cmd_env(args);
    }
    let val = shell::with_shell(|sh| sh.env.get(&args[0]).cloned());
    match val {
        Some(v) => CmdResult::ok_one(v),
        None => CmdResult { output: Vec::new(), success: false },
    }
}

fn cmd_export(args: &[String]) -> CmdResult {
    for arg in args {
        if let Some(eq) = arg.find('=') {
            let key = String::from(&arg[..eq]);
            let val = String::from(&arg[eq + 1..]);
            shell::with_shell(|sh| sh.env.insert(key, val));
        }
    }
    CmdResult::ok_empty()
}

fn cmd_unset(args: &[String]) -> CmdResult {
    for arg in args {
        shell::with_shell(|sh| sh.env.remove(arg));
    }
    CmdResult::ok_empty()
}

fn cmd_who(_args: &[String]) -> CmdResult {
    CmdResult::ok_one(String::from("root     tty0         boot"))
}

fn cmd_su(_args: &[String]) -> CmdResult {
    CmdResult::ok_one(String::from("su: already running as root"))
}

// ════════════════════════════════════════════════════════════
//  Utility commands
// ════════════════════════════════════════════════════════════

fn cmd_date(args: &[String]) -> CmdResult {
    let secs = fs::with_fs(|fs| fs.uptime_secs());
    // Simple date based on uptime
    let format_utc = args.iter().any(|a| a == "-u" || a == "--utc");
    let tz = if format_utc { "UTC" } else { "KST" };
    CmdResult::ok_one(format!("Thu Jan  1 00:00:{:02} {} 1970 (uptime: {}s)", secs % 60, tz, secs))
}

fn cmd_cal(_args: &[String]) -> CmdResult {
    let output = vec![
        String::from("    January 1970"),
        String::from("Su Mo Tu We Th Fr Sa"),
        String::from("             1  2  3"),
        String::from(" 4  5  6  7  8  9 10"),
        String::from("11 12 13 14 15 16 17"),
        String::from("18 19 20 21 22 23 24"),
        String::from("25 26 27 28 29 30 31"),
    ];
    CmdResult::ok(output)
}

fn cmd_clear(_args: &[String]) -> CmdResult {
    // Special: the terminal renderer recognises this
    CmdResult::ok(vec![String::from("\x1B[CLEAR]")])
}

fn cmd_history(_args: &[String]) -> CmdResult {
    let output = shell::with_shell(|sh| {
        sh.history.iter().enumerate()
            .map(|(i, cmd)| format!("{:>5}  {}", i + 1, cmd))
            .collect::<Vec<String>>()
    });
    CmdResult::ok(output)
}

fn cmd_alias(args: &[String]) -> CmdResult {
    if args.is_empty() {
        // List aliases
        let output = shell::with_shell(|sh| {
            sh.aliases.iter()
                .map(|(k, v)| format!("alias {}='{}'", k, v))
                .collect::<Vec<String>>()
        });
        return CmdResult::ok(output);
    }

    for arg in args {
        if let Some(eq) = arg.find('=') {
            let name = String::from(&arg[..eq]);
            let value = arg[eq + 1..].trim_matches('\'').trim_matches('"');
            shell::with_shell(|sh| sh.aliases.insert(name, String::from(value)));
        }
    }
    CmdResult::ok_empty()
}

fn cmd_which(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("which: missing argument"));
    }
    let mut output = Vec::new();
    for arg in args {
        if COMMAND_LIST.contains(&arg.as_str()) {
            output.push(format!("/usr/bin/{}", arg));
        } else {
            let is_alias = shell::with_shell(|sh| sh.aliases.contains_key(arg));
            if is_alias {
                let val = shell::with_shell(|sh| sh.aliases.get(arg).cloned().unwrap_or_default());
                output.push(format!("{}: aliased to {}", arg, val));
            } else {
                output.push(format!("{} not found", arg));
            }
        }
    }
    CmdResult::ok(output)
}

fn cmd_type(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("type: missing argument"));
    }
    let mut output = Vec::new();
    for arg in args {
        if COMMAND_LIST.contains(&arg.as_str()) {
            output.push(format!("{} is /usr/bin/{}", arg, arg));
        } else {
            let alias_val = shell::with_shell(|sh| sh.aliases.get(arg).cloned());
            if let Some(val) = alias_val {
                output.push(format!("{} is aliased to '{}'", arg, val));
            } else {
                output.push(format!("bash: type: {}: not found", arg));
            }
        }
    }
    CmdResult::ok(output)
}

fn cmd_yes(args: &[String]) -> CmdResult {
    let text = if args.is_empty() { "y" } else { &args[0] };
    // Output limited lines to avoid infinite loop
    let output: Vec<String> = (0..20).map(|_| String::from(text)).collect();
    CmdResult::ok(output)
}

fn cmd_seq(args: &[String]) -> CmdResult {
    match args.len() {
        1 => {
            let end: i64 = args[0].parse().unwrap_or(1);
            let output: Vec<String> = (1..=end).map(|n| format!("{}", n)).collect();
            CmdResult::ok(output)
        }
        2 => {
            let start: i64 = args[0].parse().unwrap_or(1);
            let end: i64 = args[1].parse().unwrap_or(1);
            let output: Vec<String> = (start..=end).map(|n| format!("{}", n)).collect();
            CmdResult::ok(output)
        }
        3 => {
            let start: i64 = args[0].parse().unwrap_or(1);
            let step: i64 = args[1].parse().unwrap_or(1);
            let end: i64 = args[2].parse().unwrap_or(1);
            if step == 0 { return CmdResult::err(String::from("seq: zero increment")); }
            let mut output = Vec::new();
            let mut n = start;
            if step > 0 {
                while n <= end { output.push(format!("{}", n)); n += step; }
            } else {
                while n >= end { output.push(format!("{}", n)); n += step; }
            }
            CmdResult::ok(output)
        }
        _ => CmdResult::err(String::from("seq: missing operand")),
    }
}

fn cmd_sleep(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("sleep: missing operand"));
    }
    let _secs: u64 = args[0].parse().unwrap_or(0);
    // In a real OS we'd sleep; here we just acknowledge
    CmdResult::ok_empty()
}

fn cmd_time(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::ok_empty();
    }
    let result = execute_command(&args[0], &args[1..].to_vec(), None);
    let mut output = result.output;
    output.push(String::new());
    output.push(String::from("real\t0m0.001s"));
    output.push(String::from("user\t0m0.000s"));
    output.push(String::from("sys\t0m0.001s"));
    CmdResult::ok(output)
}

fn cmd_test(args: &[String]) -> CmdResult {
    // Basic test: -f file, -d dir, -z string, -n string, = !=
    if args.is_empty() {
        return CmdResult { output: Vec::new(), success: false };
    }

    let success = match args[0].as_str() {
        "-f" if args.len() > 1 => {
            let abs = shell::with_shell(|sh| sh.resolve_path(&args[1]));
            fs::with_fs(|fs| fs.resolve(&abs).and_then(|ino| fs.get(ino).map(|n| n.mode.is_file())).unwrap_or(false))
        }
        "-d" if args.len() > 1 => {
            let abs = shell::with_shell(|sh| sh.resolve_path(&args[1]));
            fs::with_fs(|fs| fs.resolve(&abs).and_then(|ino| fs.get(ino).map(|n| n.mode.is_dir())).unwrap_or(false))
        }
        "-e" if args.len() > 1 => {
            let abs = shell::with_shell(|sh| sh.resolve_path(&args[1]));
            fs::with_fs(|fs| fs.resolve(&abs).is_some())
        }
        "-z" if args.len() > 1 => args[1].is_empty(),
        "-n" if args.len() > 1 => !args[1].is_empty(),
        _ if args.len() == 3 => {
            match args[1].as_str() {
                "=" | "==" => args[0] == args[2],
                "!=" => args[0] != args[2],
                _ => false,
            }
        }
        _ => !args[0].is_empty(),
    };

    CmdResult { output: Vec::new(), success }
}

fn cmd_exit(_args: &[String]) -> CmdResult {
    CmdResult::ok_one(String::from("exit: cannot exit — this is the only shell"))
}

// ════════════════════════════════════════════════════════════
//  Network (stubs)
// ════════════════════════════════════════════════════════════

fn cmd_ping(args: &[String]) -> CmdResult {
    let host = args.first().map(|s| s.as_str()).unwrap_or("localhost");
    // Resolve via DNS
    let ip_str = match crate::net::dns::resolve(host) {
        Ok(entry) => {
            if let Some(addr) = entry.addresses.first() {
                format!("{}", addr)
            } else {
                String::from("127.0.0.1")
            }
        }
        Err(_) => {
            return CmdResult::err(format!("ping: {}: Name or service not known", host));
        }
    };
    // Record loopback traffic
    crate::net::loopback_transfer(84 * 3);
    let output = vec![
        format!("PING {} ({}) 56(84) bytes of data.", host, ip_str),
        format!("64 bytes from {} ({}): icmp_seq=1 ttl=64 time=0.04 ms", host, ip_str),
        format!("64 bytes from {} ({}): icmp_seq=2 ttl=64 time=0.03 ms", host, ip_str),
        format!("64 bytes from {} ({}): icmp_seq=3 ttl=64 time=0.03 ms", host, ip_str),
        String::new(),
        format!("--- {} ping statistics ---", host),
        String::from("3 packets transmitted, 3 received, 0% packet loss, time 2ms"),
        String::from("rtt min/avg/max/mdev = 0.030/0.033/0.040/0.005 ms"),
    ];
    CmdResult::ok(output)
}

fn cmd_ifconfig(_args: &[String]) -> CmdResult {
    let ifaces = crate::net::interfaces();
    let mut output = Vec::new();
    for iface in &ifaces {
        let flags = if iface.up { "UP,LOOPBACK,RUNNING" } else { "DOWN" };
        output.push(format!("{}: flags=73<{}>  mtu {}", iface.name, flags, iface.mtu));
        output.push(format!("        inet {}  netmask {}", iface.ip, iface.netmask));
        output.push(format!(
            "        RX packets {}  bytes {} ({} B)",
            iface.rx_packets, iface.rx_bytes, iface.rx_bytes
        ));
        output.push(format!(
            "        TX packets {}  bytes {} ({} B)",
            iface.tx_packets, iface.tx_bytes, iface.tx_bytes
        ));
        output.push(String::new());
    }
    CmdResult::ok(output)
}

fn cmd_netstat(_args: &[String]) -> CmdResult {
    let conns = crate::net::tcp::connections();
    let mut output = vec![
        String::from("Active Internet connections (servers and established)"),
        String::from("Proto Recv-Q Send-Q Local Address           Foreign Address         State"),
    ];
    for (id, state, local, remote) in &conns {
        output.push(format!(
            "tcp    0      0 {:<23} {:<23} {}",
            format!("{}", local),
            format!("{}", remote),
            state.as_str()
        ));
    }
    if conns.is_empty() {
        output.push(String::from("  (no active connections)"));
    }
    output.push(format!(
        "\nTotal connections created: {}",
        crate::net::tcp::total_connections()
    ));
    CmdResult::ok(output)
}

// ════════════════════════════════════════════════════════════
//  Hardware commands
// ════════════════════════════════════════════════════════════

fn cmd_lspci(_args: &[String]) -> CmdResult {
    let devices = crate::driver::pci::devices();
    let mut output = Vec::new();
    if devices.is_empty() {
        output.push(String::from("No PCI devices found"));
    } else {
        output.push(format!("PCI devices ({} found):", devices.len()));
        for dev in &devices {
            output.push(format!(
                "  {:02x}:{:02x}.{} [{:04x}:{:04x}] class {}",
                dev.address.bus, dev.address.device, dev.address.function,
                dev.vendor_id, dev.device_id, dev.class
            ));
        }
    }
    CmdResult::ok(output)
}

fn cmd_lsblk(_args: &[String]) -> CmdResult {
    let devs = crate::driver::virtio::block::device_info();
    let mut output = Vec::new();
    output.push(String::from("NAME       SIZE  TYPE"));
    if devs.is_empty() {
        output.push(String::from("(no block devices)"));
    } else {
        for (idx, sectors, mb) in &devs {
            output.push(format!(
                "vda{}      {}M  VirtIO ({} sectors)",
                idx, mb, sectors
            ));
        }
    }
    CmdResult::ok(output)
}

fn cmd_acpi(_args: &[String]) -> CmdResult {
    let count = crate::hw::acpi::table_count();
    let mut output = Vec::new();
    if count == 0 {
        output.push(String::from("ACPI: not available (no RSDP or tables not parsed)"));
    } else {
        output.push(format!("ACPI tables ({}):", count));
        for sig in crate::hw::acpi::table_signatures() {
            output.push(format!("  {}", sig));
        }
        let lapic = crate::hw::acpi::local_apic_count();
        let ioapic = crate::hw::acpi::io_apic_count();
        if lapic > 0 || ioapic > 0 {
            output.push(format!("MADT: {} Local APIC(s), {} I/O APIC(s)", lapic, ioapic));
        }
    }
    CmdResult::ok(output)
}

// ════════════════════════════════════════════════════════════
//  Misc commands
// ════════════════════════════════════════════════════════════

fn cmd_man(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("What manual page do you want?"));
    }

    let cmd = &args[0];
    let desc = match cmd.as_str() {
        "ls" => "ls - list directory contents\n\nSYNOPSIS: ls [OPTION]... [FILE]...\n\nOPTIONS:\n  -a  do not ignore entries starting with .\n  -l  use a long listing format\n  -h  human readable sizes",
        "cd" => "cd - change the shell working directory\n\nSYNOPSIS: cd [dir]\n\n  cd -     go to previous directory\n  cd ~     go to home directory",
        "cat" => "cat - concatenate files and print on the standard output\n\nSYNOPSIS: cat [OPTION]... [FILE]...\n\nOPTIONS:\n  -n  number all output lines",
        "grep" => "grep - print lines that match patterns\n\nSYNOPSIS: grep [OPTION]... PATTERN [FILE]...\n\nOPTIONS:\n  -i  ignore case\n  -v  invert match\n  -c  count only\n  -n  line numbers",
        "find" => "find - search for files in a directory hierarchy\n\nSYNOPSIS: find [path] [expression]\n\nOPTIONS:\n  -name PATTERN  match filename\n  -type d/f      match directory/file",
        _ => "No manual entry found. Try: help",
    };

    let lines: Vec<String> = desc.lines().map(String::from).collect();
    CmdResult::ok(lines)
}

fn cmd_help(_args: &[String]) -> CmdResult {
    let output = vec![
        String::from("KPIO Shell — Built-in Commands"),
        String::from("══════════════════════════════════════════════════════════"),
        String::new(),
        String::from("Filesystem:"),
        String::from("  ls cd pwd mkdir rmdir touch rm cp mv cat head tail"),
        String::from("  find du df stat file tree wc ln chmod chown basename dirname"),
        String::new(),
        String::from("Text Processing:"),
        String::from("  echo printf grep sed sort uniq cut tr tee xargs tac nl od"),
        String::new(),
        String::from("System Info:"),
        String::from("  uname whoami hostname uptime free top ps kill id groups"),
        String::from("  env printenv export unset who"),
        String::new(),
        String::from("Utilities:"),
        String::from("  date cal clear history alias which type true false yes seq"),
        String::from("  sleep time test exit"),
        String::new(),
        String::from("Network:"),
        String::from("  ping ifconfig netstat"),
        String::new(),
        String::from("Hardware:"),
        String::from("  lspci lsblk acpi"),
        String::new(),
        String::from("Misc:"),
        String::from("  man help neofetch hexdump base64 md5sum sha256sum diff"),
        String::new(),
        String::from("Shell Features:"),
        String::from("  Pipes: cmd1 | cmd2          Variable expansion: $VAR ${VAR}"),
        String::from("  Redirect: > >> <            Chaining: ; && ||"),
        String::from("  Quoting: \"double\" 'single'  History: up/down arrows"),
    ];
    CmdResult::ok(output)
}

fn cmd_neofetch(_args: &[String]) -> CmdResult {
    let secs = fs::with_fs(|fs| fs.uptime_secs());
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;

    let output = vec![
        String::from("        ████████████          root@kpio"),
        String::from("      ██            ██        ─────────────────"),
        String::from("    ██   ██████████   ██      OS: KPIO OS 1.0.0 x86_64"),
        String::from("   ██  ██         ██  ██      Kernel: 1.0.0"),
        format!("  ██  ██    KPIO    ██  ██      Uptime: {}h {}m", hours, mins),
        String::from("  ██  ██           ██  ██      Shell: kpio-sh"),
        String::from("  ██  ██           ██  ██      Terminal: kpio-term"),
        String::from("   ██  ██         ██  ██      CPU: KPIO Virtual CPU @ 2000MHz"),
        String::from("    ██   ██████████   ██      Memory: 64MiB / 500MiB"),
        String::from("      ██            ██"),
        String::from("        ████████████          ████████████████████"),
    ];
    CmdResult::ok(output)
}

fn cmd_hexdump(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("hexdump: missing file operand"));
    }

    let path = args.last().unwrap();
    let abs = shell::with_shell(|sh| sh.resolve_path(path));
    let ino = match fs::with_fs(|fs| fs.resolve(&abs)) {
        Some(i) => i,
        None => return CmdResult::err(format!("hexdump: {}: No such file or directory", path)),
    };
    let data = match fs::with_fs(|fs| fs.read_file(ino)) {
        Ok(d) => d,
        Err(e) => return CmdResult::err(format!("hexdump: {}", e.as_str())),
    };

    let mut output = Vec::new();
    for (offset, chunk) in data.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
        let ascii: String = chunk.iter().map(|&b| {
            if b >= 0x20 && b < 0x7f { b as char } else { '.' }
        }).collect();
        let hex_str = format!("{:<48}", hex.join(" "));
        output.push(format!("{:08x}  {}  |{}|", offset * 16, hex_str, ascii));
    }
    output.push(format!("{:08x}", data.len()));
    CmdResult::ok(output)
}

fn cmd_base64(args: &[String]) -> CmdResult {
    let decode = args.iter().any(|a| a == "-d" || a == "--decode");
    let path = args.iter().find(|a| !a.starts_with('-'));

    if let Some(path) = path {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        let data = match read_file_bytes(&abs) {
            Ok(d) => d,
            Err(e) => return CmdResult::err(e),
        };

        if decode {
            CmdResult::ok_one(String::from("[base64 decode: not implemented in no_std]"))
        } else {
            CmdResult::ok_one(simple_base64_encode(&data))
        }
    } else {
        CmdResult::err(String::from("base64: missing file operand"))
    }
}

fn cmd_md5sum(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("md5sum: missing file operand"));
    }
    let mut output = Vec::new();
    for path in args {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        match read_file_bytes(&abs) {
            Ok(data) => {
                let hash = simple_hash(&data);
                output.push(format!("{}  {}", hash, path));
            }
            Err(e) => output.push(format!("md5sum: {}: {}", path, e)),
        }
    }
    CmdResult::ok(output)
}

fn cmd_sha256sum(args: &[String]) -> CmdResult {
    if args.is_empty() {
        return CmdResult::err(String::from("sha256sum: missing file operand"));
    }
    let mut output = Vec::new();
    for path in args {
        let abs = shell::with_shell(|sh| sh.resolve_path(path));
        match read_file_bytes(&abs) {
            Ok(data) => {
                let hash = simple_hash_64(&data);
                output.push(format!("{}  {}", hash, path));
            }
            Err(e) => output.push(format!("sha256sum: {}: {}", path, e)),
        }
    }
    CmdResult::ok(output)
}

fn cmd_diff(args: &[String]) -> CmdResult {
    if args.len() < 2 {
        return CmdResult::err(String::from("diff: missing operand"));
    }

    let abs1 = shell::with_shell(|sh| sh.resolve_path(&args[0]));
    let abs2 = shell::with_shell(|sh| sh.resolve_path(&args[1]));

    let lines1 = match read_file_lines(&abs1) {
        Ok(l) => l,
        Err(e) => return CmdResult::err(format!("diff: {}: {}", args[0], e)),
    };
    let lines2 = match read_file_lines(&abs2) {
        Ok(l) => l,
        Err(e) => return CmdResult::err(format!("diff: {}: {}", args[1], e)),
    };

    if lines1 == lines2 {
        return CmdResult::ok_empty();
    }

    let mut output = Vec::new();
    let max = core::cmp::max(lines1.len(), lines2.len());
    for i in 0..max {
        let l1 = lines1.get(i);
        let l2 = lines2.get(i);
        match (l1, l2) {
            (Some(a), Some(b)) if a != b => {
                output.push(format!("{}c{}", i + 1, i + 1));
                output.push(format!("< {}", a));
                output.push(String::from("---"));
                output.push(format!("> {}", b));
            }
            (Some(a), None) => {
                output.push(format!("{}d{}", i + 1, lines2.len()));
                output.push(format!("< {}", a));
            }
            (None, Some(b)) => {
                output.push(format!("{}a{}", lines1.len(), i + 1));
                output.push(format!("> {}", b));
            }
            _ => {}
        }
    }

    CmdResult { output, success: false } // diff returns 1 when files differ
}

// ════════════════════════════════════════════════════════════
//  Helpers
// ════════════════════════════════════════════════════════════

fn read_file_content(abs_path: &str) -> Result<String, String> {
    let ino = fs::with_fs(|fs| fs.resolve(abs_path))
        .ok_or_else(|| format!("No such file or directory"))?;
    let data = fs::with_fs(|fs| fs.read_file(ino))
        .map_err(|e| String::from(e.as_str()))?;
    Ok(String::from_utf8_lossy(&data).into_owned())
}

fn read_file_lines(abs_path: &str) -> Result<Vec<String>, String> {
    let text = read_file_content(abs_path)?;
    Ok(text.lines().map(String::from).collect())
}

fn read_file_bytes(abs_path: &str) -> Result<Vec<u8>, String> {
    let ino = fs::with_fs(|fs| fs.resolve(abs_path))
        .ok_or_else(|| String::from("No such file or directory"))?;
    fs::with_fs(|fs| fs.read_file(ino))
        .map_err(|e| String::from(e.as_str()))
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 { return format!("{}B", bytes); }
    if bytes < 1024 * 1024 { return format!("{:.1}K", bytes as f64 / 1024.0); }
    if bytes < 1024 * 1024 * 1024 { return format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0)); }
    format!("{:.1}G", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

fn simple_glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" { return true; }
    if pattern.starts_with("*.") {
        let ext = &pattern[1..]; // ".ext"
        return name.ends_with(ext);
    }
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return name.starts_with(prefix);
    }
    name == pattern
}

fn simple_base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        result.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < data.len() {
            result.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else { result.push('='); }
        if i + 2 < data.len() {
            result.push(TABLE[(triple & 0x3F) as usize] as char);
        } else { result.push('='); }
        i += 3;
    }
    result
}

/// Simple deterministic hash for display purposes (not crypto!)
fn simple_hash(data: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}{:016x}", h, h.wrapping_mul(0x9e3779b97f4a7c15))
}

fn simple_hash_64(data: &[u8]) -> String {
    let mut h1: u64 = 0x6a09e667f3bcc908;
    let mut h2: u64 = 0xbb67ae8584caa73b;
    let mut h3: u64 = 0x3c6ef372fe94f82b;
    let mut h4: u64 = 0xa54ff53a5f1d36f1;
    for &b in data {
        h1 = h1.wrapping_add(b as u64).wrapping_mul(0x100000001b3);
        h2 = h2.wrapping_add(b as u64).wrapping_mul(0x9e3779b97f4a7c15);
        h3 ^= h1;
        h4 ^= h2;
    }
    format!("{:016x}{:016x}{:016x}{:016x}", h1, h2, h3, h4)
}
