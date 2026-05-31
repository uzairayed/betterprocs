//! System-wide process search backed by `ps`, exposed to the MCP server so an
//! agent can find and kill arbitrary OS processes. Shelling out to `ps` keeps
//! this dependency-free, matching the `lsof`-based approach used elsewhere.

use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub command: String,
}

/// Search running processes whose name or full command contains `query`
/// (case-insensitive). An empty query returns everything.
pub fn search_processes(query: &str) -> Vec<ProcInfo> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,comm=,args="])
        .output();

    let stdout = match output {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&stdout);
    filter_processes(parse_ps_output(&text), query)
}

/// Parse `ps -axo pid=,comm=,args=` output. Each line is:
/// `<pid> <comm> <args...>` with whitespace separators. `comm` may contain a
/// path; `args` is the full command line (may contain spaces).
pub fn parse_ps_output(text: &str) -> Vec<ProcInfo> {
    let mut procs = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if line.is_empty() {
            continue;
        }
        // pid is the first whitespace-delimited token.
        let mut parts = line.splitn(2, char::is_whitespace);
        let pid = match parts.next().and_then(|p| p.trim().parse::<u32>().ok()) {
            Some(pid) => pid,
            None => continue,
        };
        let rest = parts.next().unwrap_or("").trim_start();
        // comm is the next token; the remainder is the full args string.
        let mut rest_parts = rest.splitn(2, char::is_whitespace);
        let comm = rest_parts.next().unwrap_or("").to_string();
        let args = rest_parts.next().unwrap_or("").trim_start().to_string();
        // The short name is the basename of comm.
        let name = comm
            .rsplit('/')
            .next()
            .unwrap_or(&comm)
            .to_string();
        let command = if args.is_empty() {
            comm.clone()
        } else {
            format!("{} {}", comm, args)
        };
        procs.push(ProcInfo { pid, name, command });
    }
    procs
}

fn filter_processes(procs: Vec<ProcInfo>, query: &str) -> Vec<ProcInfo> {
    if query.is_empty() {
        return procs;
    }
    let q = query.to_lowercase();
    procs
        .into_iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&q) || p.command.to_lowercase().contains(&q)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pid_name_and_command() {
        let sample = "  123 /usr/bin/node node server.js --port 3000\n\
                       456 zsh -zsh\n";
        let procs = parse_ps_output(sample);
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0].pid, 123);
        assert_eq!(procs[0].name, "node");
        assert_eq!(procs[0].command, "/usr/bin/node node server.js --port 3000");
        assert_eq!(procs[1].pid, 456);
        assert_eq!(procs[1].name, "zsh");
    }

    #[test]
    fn skips_garbage_lines() {
        let sample = "not-a-pid here\n789 cargo cargo run\n\n";
        let procs = parse_ps_output(sample);
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].pid, 789);
    }

    #[test]
    fn filter_matches_name_or_command_case_insensitive() {
        let procs = vec![
            ProcInfo { pid: 1, name: "node".into(), command: "/usr/bin/node app.js".into() },
            ProcInfo { pid: 2, name: "zsh".into(), command: "-zsh".into() },
        ];
        let hits = filter_processes(procs.clone(), "NODE");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pid, 1);

        // Match on command substring even when name doesn't match.
        let hits = filter_processes(procs.clone(), "app.js");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pid, 1);

        // Empty query returns all.
        assert_eq!(filter_processes(procs, "").len(), 2);
    }
}
