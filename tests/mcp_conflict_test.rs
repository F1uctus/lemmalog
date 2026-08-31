//! One snapshot, two servers: the second writer must not erase the first.
#![cfg(feature = "mcp")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};

struct Srv {
    child: Child,
    out: BufReader<ChildStdout>,
}

fn srv(path: &str) -> Srv {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lemmalog-mcp"))
        .env("LEMMALOG_MCP_PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn lemmalog-mcp");
    let out = BufReader::new(child.stdout.take().unwrap());
    let mut s = Srv { child, out };
    s.rpc(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    s
}

impl Srv {
    fn rpc(&mut self, msg: &str) -> String {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        line
    }
    fn observe(&mut self, facts: &str) -> String {
        self.rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"lemmalog_observe","arguments":{{"facts":"{facts}","ts":100}}}}}}"#
        ))
    }
}

#[test]
fn second_writer_is_refused_not_silently_merged() {
    let dir = std::env::temp_dir().join("lemmalog-mcp-conflict");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("mem-{}.snapshot", std::process::id()));
    let path = path.to_str().unwrap().to_string();
    let _ = std::fs::remove_file(&path);

    // seed the file so both servers load the same starting point
    let mut seed = srv(&path);
    seed.observe("origin --wrote--> seed");
    seed.child.kill().ok();

    let mut a = srv(&path);
    let mut b = srv(&path);

    let ra = a.observe("agent_a --wrote--> first");
    assert!(!ra.contains("NOT PERSISTED"), "first writer should persist: {ra}");
    // filesystem mtime granularity: make sure the second save sees a change
    std::thread::sleep(std::time::Duration::from_millis(20));
    a.observe("agent_a --wrote_again--> second");

    let rb = b.observe("agent_b --wrote--> third");
    assert!(rb.contains("NOT PERSISTED"), "second writer must be refused: {rb}");
    assert!(rb.contains("conflict:"), "refusal must name the cause: {rb}");

    let disk = std::fs::read_to_string(&path).unwrap();
    assert!(disk.contains("agent_a"), "first writer's work must survive");
    assert!(!disk.contains("agent_b"), "second writer must not have clobbered the file");

    // and the refused work is parked, not thrown away
    let parked: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&format!("mem-{}.snapshot.conflict-", std::process::id())))
        .collect();
    assert_eq!(parked.len(), 1, "refused write should be parked beside the snapshot");

    a.child.kill().ok();
    b.child.kill().ok();
}

#[test]
fn server_started_without_a_snapshot_does_not_flatten_one_that_appears() {
    // The (mtime, len) guard skipped this case entirely: a server that came up
    // before the snapshot existed had `seen == None`, so its first save
    // overwrote whatever another agent had created in the meantime. That is
    // how a session can silently lose rule batches it never knew about.
    let dir = std::env::temp_dir().join("lemmalog-mcp-appear");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("mem-{}.snapshot", std::process::id()));
    let path = path.to_str().unwrap().to_string();
    let _ = std::fs::remove_file(&path);

    // A starts with no file on disk.
    let mut a = srv(&path);

    // B creates the snapshot in the meantime.
    let mut b = srv(&path);
    b.observe("other_agent --wrote--> important");
    b.child.kill().ok();
    assert!(std::fs::read_to_string(&path).unwrap().contains("other_agent"));

    // A must now refuse rather than replace B's file.
    let ra = a.observe("late_starter --wrote--> mine");
    assert!(
        ra.contains("NOT PERSISTED"),
        "a server that started before the file existed must not overwrite it: {ra}"
    );

    let disk = std::fs::read_to_string(&path).unwrap();
    assert!(disk.contains("other_agent"), "the earlier writer's work must survive");
    assert!(!disk.contains("late_starter"), "the late starter must not have clobbered it");

    a.child.kill().ok();
}
