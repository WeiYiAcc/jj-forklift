use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const CONFIG_PREFIX: &str = "stack";
const STACK_FIELD_SEPARATOR: char = '\x1f';
const STACK_RECORD_SEPARATOR: char = '\x1e';
const ROOT_PARENT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REMOTE_TRUNK: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const COMMIT_ONE: &str = "1111111111111111111111111111111111111111";
const COMMIT_TWO: &str = "2222222222222222222222222222222222222222";

fn main() -> Result<()> {
    let scenario = env::args()
        .nth(1)
        .unwrap_or_else(|| "merge-two-prs-settle-slow".to_owned());
    match scenario.as_str() {
        "merge-two-prs-settle-slow" => run_merge_scenario(Scenario::SettleSlow),
        "verify-merge-slow" => run_merge_scenario(Scenario::VerifySlow),
        "cleanup-branches" => run_merge_scenario(Scenario::CleanupBranches),
        "list" | "--list" | "-l" => {
            println!("merge-two-prs-settle-slow");
            println!("verify-merge-slow");
            println!("cleanup-branches");
            Ok(())
        }
        other => {
            bail!(
                "unknown scenario `{other}`\nknown scenarios: merge-two-prs-settle-slow, verify-merge-slow, cleanup-branches"
            )
        }
    }
}

#[derive(Clone, Copy)]
enum Scenario {
    SettleSlow,
    VerifySlow,
    CleanupBranches,
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::SettleSlow => "merge-two-prs-settle-slow",
            Self::VerifySlow => "verify-merge-slow",
            Self::CleanupBranches => "cleanup-branches",
        }
    }
}

fn run_merge_scenario(scenario: Scenario) -> Result<()> {
    let fixture = Fixture::new(scenario.name())?;
    fixture.seed_two_pr_merge(scenario)?;

    eprintln!("ui-scenario: {}", scenario.name());
    eprintln!("fixture: {}", fixture.root.display());
    eprintln!("running: jj-stack merge --admin");

    let status = fixture.run_jj_stack(["merge", "--admin"])?;
    if !status.success() {
        bail!("scenario `{}` failed with status {status}", scenario.name());
    }

    eprintln!(
        "scenario complete; fixture kept at {}",
        fixture.root.display()
    );
    Ok(())
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    repo_dir: PathBuf,
    bin_dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self> {
        let root = unique_dir(name)?;
        let workspace = root.join("workspace");
        let repo_dir = workspace.join(".jj").join("repo");
        let bin_dir = root.join("bin");
        fs::create_dir_all(&repo_dir)?;
        fs::create_dir_all(&bin_dir)?;
        let fixture = Self {
            root,
            workspace,
            repo_dir,
            bin_dir,
        };
        fixture.install_fakes()?;
        Ok(fixture)
    }

    fn install_fakes(&self) -> Result<()> {
        write_executable(&self.bin_dir.join("jj"), JJ_FAKE)?;
        write_executable(&self.bin_dir.join("git"), GIT_FAKE)?;
        write_executable(&self.bin_dir.join("gh"), GH_FAKE)
    }

    fn seed_two_pr_merge(&self, scenario: Scenario) -> Result<()> {
        let bottom_branch = "stack/bottom-title-bottomch";
        let top_branch = "stack/top-title-topchang";
        self.write_jj_logs(&[stack_log(&[
            change_record(
                "bottomchange",
                COMMIT_ONE,
                &[ROOT_PARENT],
                "bottom title",
                "bottom title\n\nbottom body",
            ),
            change_record(
                "topchange",
                COMMIT_TWO,
                &[COMMIT_ONE],
                "top title",
                "top title\n\ntop body",
            ),
        ])])?;
        self.write_git_maps(
            [("main", ROOT_PARENT), ("origin/main", REMOTE_TRUNK)],
            [
                (ROOT_PARENT, REMOTE_TRUNK, true),
                (REMOTE_TRUNK, COMMIT_TWO, true),
            ],
        )?;
        self.write_branch_heads([
            ("main", ROOT_PARENT),
            (bottom_branch, COMMIT_ONE),
            (top_branch, COMMIT_TWO),
        ])?;
        self.write_local_bookmarks([
            ("main", ROOT_PARENT),
            (bottom_branch, COMMIT_ONE),
            (top_branch, COMMIT_TWO),
        ])?;
        self.write_prs([
            merge_pr_json(11, "OPEN", bottom_branch, "main", COMMIT_ONE, ROOT_PARENT),
            merge_pr_json(
                12,
                "OPEN",
                top_branch,
                bottom_branch,
                COMMIT_TWO,
                COMMIT_ONE,
            ),
        ])?;
        self.write_cache(
            "bottomchange",
            11,
            bottom_branch,
            "main",
            COMMIT_ONE,
            ROOT_PARENT,
            "bottom title",
            "bottom body",
        )?;
        self.write_cache(
            "topchange",
            12,
            top_branch,
            bottom_branch,
            COMMIT_TWO,
            COMMIT_ONE,
            "top title",
            "top body",
        )?;

        fs::write(
            self.root.join("scenario.json"),
            serde_json::to_string(scenario.name())?,
        )?;
        fs::write(
            self.root.join("auto-merge-trunk.json"),
            serde_json::to_string("main")?,
        )?;
        if matches!(scenario, Scenario::CleanupBranches) {
            fs::write(
                self.root.join("cleanup-stack-bookmarks.json"),
                serde_json::to_string(&vec![bottom_branch, top_branch])?,
            )?;
        }
        Ok(())
    }

    fn run_jj_stack<const N: usize>(&self, args: [&str; N]) -> Result<std::process::ExitStatus> {
        let old_path = env::var("PATH").unwrap_or_default();
        let mut command = Command::new(resolve_jj_stack_bin()?);
        command
            .args(args)
            .current_dir(&self.workspace)
            .env("PATH", format!("{}:{old_path}", self.bin_dir.display()))
            .env("STACK_FAKE_ROOT", &self.root)
            .env("STACK_FAKE_WORKSPACE", &self.workspace)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        command.status().map_err(Into::into)
    }

    fn write_jj_logs(&self, logs: &[String]) -> Result<()> {
        for (index, log) in logs.iter().enumerate() {
            fs::write(self.root.join(format!("jj-log-{}", index + 1)), log)?;
        }
        Ok(())
    }

    fn write_git_maps<const R: usize, const A: usize>(
        &self,
        rev_parse: [(&str, &str); R],
        ancestors: [(&str, &str, bool); A],
    ) -> Result<()> {
        fs::write(
            self.root.join("rev-parse.json"),
            serde_json::to_string(&rev_parse.into_iter().collect::<BTreeMap<_, _>>())?,
        )?;
        let ancestors = ancestors
            .into_iter()
            .map(|(left, right, success)| (format!("{left}\n{right}"), success))
            .collect::<BTreeMap<_, _>>();
        fs::write(
            self.root.join("ancestors.json"),
            serde_json::to_string(&ancestors)?,
        )?;
        Ok(())
    }

    fn write_branch_heads<const N: usize>(&self, heads: [(&str, &str); N]) -> Result<()> {
        fs::write(
            self.root.join("branch-heads.json"),
            serde_json::to_string(&heads.into_iter().collect::<BTreeMap<_, _>>())?,
        )?;
        Ok(())
    }

    fn write_local_bookmarks<const N: usize>(&self, heads: [(&str, &str); N]) -> Result<()> {
        fs::write(
            self.root.join("local-bookmarks.json"),
            serde_json::to_string(&heads.into_iter().collect::<BTreeMap<_, _>>())?,
        )?;
        Ok(())
    }

    fn write_prs<const N: usize>(&self, prs: [Value; N]) -> Result<()> {
        fs::write(
            self.root.join("prs.json"),
            serde_json::to_string(&prs.into_iter().collect::<Vec<_>>())?,
        )?;
        Ok(())
    }

    fn write_cache(
        &self,
        change_id: &str,
        pr_number: u64,
        head_branch: &str,
        base_branch: &str,
        head_sha: &str,
        base_sha: &str,
        title: &str,
        body: &str,
    ) -> Result<()> {
        let path = self.repo_dir.join(CONFIG_PREFIX).join("cache.sqlite");
        let parent = path
            .parent()
            .with_context(|| format!("cache path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent)?;
        let conn = Connection::open(path)?;
        init_cache_schema(&conn)?;
        conn.execute(
            "INSERT OR REPLACE INTO pr_cache (
                repo, change_id, pr_number, pr_node_id, head_branch, base_branch, base_ref,
                head_repo_id, head_repo_node_id, head_repo_name, base_repo_id,
                base_repo_node_id, base_repo_name, head_sha, base_sha, author_login, title,
                body, created_at, stack_comment_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                      ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                "owner/repo",
                change_id,
                pr_number as i64,
                format!("PR_node_{pr_number}"),
                head_branch,
                base_branch,
                base_branch,
                "repo-id",
                "repo-node",
                "owner/repo",
                "repo-id",
                "repo-node",
                "owner/repo",
                head_sha,
                base_sha,
                "octocat",
                title,
                body,
                "2026-06-03T12:34:56Z",
                "comment-101",
            ],
        )?;
        Ok(())
    }
}

fn resolve_jj_stack_bin() -> Result<PathBuf> {
    if let Some(path) = env::var_os("JJ_STACK_BIN") {
        let path = PathBuf::from(path);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Ok(env::current_dir()?.join(path))
        };
    }
    let current = env::current_exe()?;
    if let Some(dir) = current.parent() {
        let sibling = dir.join("jj-stack");
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    Ok(PathBuf::from("jj-stack"))
}

fn init_cache_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS pr_cache (
            repo TEXT NOT NULL,
            change_id TEXT NOT NULL,
            pr_number INTEGER NOT NULL,
            pr_node_id TEXT NOT NULL DEFAULT '',
            head_branch TEXT NOT NULL,
            base_branch TEXT NOT NULL,
            base_ref TEXT NOT NULL,
            head_repo_id TEXT NOT NULL DEFAULT '',
            head_repo_node_id TEXT NOT NULL DEFAULT '',
            head_repo_name TEXT NOT NULL DEFAULT '',
            base_repo_id TEXT NOT NULL DEFAULT '',
            base_repo_node_id TEXT NOT NULL DEFAULT '',
            base_repo_name TEXT NOT NULL DEFAULT '',
            head_sha TEXT NOT NULL,
            base_sha TEXT NOT NULL,
            author_login TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            body TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT '',
            stack_comment_id TEXT,
            PRIMARY KEY (repo, change_id)
        );",
    )?;
    Ok(())
}

fn write_executable(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

fn unique_dir(name: &str) -> Result<PathBuf> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = env::temp_dir().join(format!("jj-stack-ui-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn stack_log(records: &[String]) -> String {
    records.join("")
}

fn change_record(
    change_id: &str,
    commit_id: &str,
    parent_ids: &[&str],
    title: &str,
    description: &str,
) -> String {
    [
        serde_json::to_string(change_id).unwrap(),
        serde_json::to_string(commit_id).unwrap(),
        serde_json::to_string(parent_ids).unwrap(),
        serde_json::to_string(title).unwrap(),
        serde_json::to_string(description).unwrap(),
        "false".to_owned(),
        "false".to_owned(),
    ]
    .join(&STACK_FIELD_SEPARATOR.to_string())
        + &STACK_RECORD_SEPARATOR.to_string()
}

fn merge_pr_json(
    number: u64,
    state: &str,
    head: &str,
    base: &str,
    head_sha: &str,
    base_sha: &str,
) -> Value {
    json!({
        "number": number,
        "state": state,
        "id": format!("PR_node_{number}"),
        "headRefName": head,
        "baseRefName": base,
        "headRefOid": head_sha,
        "baseRefOid": base_sha,
        "headRepository": {
            "id": "repo-id",
            "node_id": "repo-node",
            "nameWithOwner": "owner/repo",
        },
        "baseRepository": {
            "id": "repo-id",
            "node_id": "repo-node",
            "nameWithOwner": "owner/repo",
        },
        "author": { "login": "octocat" },
        "title": "change title",
        "body": "change body",
        "createdAt": "2026-06-03T12:34:56Z",
        "isDraft": false,
        "reviewDecision": "APPROVED",
        "mergeable": "MERGEABLE",
        "mergeStateStatus": "CLEAN",
        "statusCheckRollup": [{ "context": "ci", "state": "SUCCESS" }],
        "autoMergeRequest": null,
    })
}

const JJ_FAKE: &str = r#"#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

root = Path(os.environ["STACK_FAKE_ROOT"])
args = sys.argv[1:]
with (root / "jj-requests.jsonl").open("a") as fh:
    fh.write(json.dumps({"args": args}) + "\n")

def load(name, default):
    path = root / name
    return json.loads(path.read_text()) if path.exists() else default

def save(name, value):
    (root / name).write_text(json.dumps(value))

if args[:2] == ["config", "get"]:
    if len(args) == 3 and args[2] == 'revset-aliases."immutable_heads()"':
        print("builtin_immutable_heads()")
        sys.exit(0)
    sys.exit(1)
if args[:3] == ["config", "set", "--repo"]:
    sys.exit(0)
if args == ["root"]:
    print(os.environ["STACK_FAKE_WORKSPACE"])
    sys.exit(0)
if args and args[0] == "log":
    if "-T" in args and "-r" in args:
        template = args[args.index("-T") + 1]
        commit_id_template = template in {"commit_id", 'commit_id ++ "\\n"'}
    else:
        commit_id_template = False
    if commit_id_template:
        rev = args[args.index("-r") + 1]
        if rev == "trunk()":
            refs = load("rev-parse.json", {})
            heads = load("branch-heads.json", {})
            print(refs.get("origin/main") or refs.get("main") or heads.get("main"))
            sys.exit(0)
        if "@" in rev:
            trunk, remote = rev.split("@", 1)
            refs = load("rev-parse.json", {})
            value = refs.get(remote + "/" + trunk)
            if value is None:
                print("missing remote bookmark " + rev, file=sys.stderr)
                sys.exit(1)
            print(value)
            sys.exit(0)
        heads = load("local-bookmarks.json", {})
        value = heads.get(rev)
        if value is None:
            print("missing local bookmark " + rev, file=sys.stderr)
            sys.exit(1)
        print(value)
        sys.exit(0)
    path = root / "jj-log-1"
    if path.exists():
        sys.stdout.write(path.read_text())
    sys.exit(0)
if args[:2] == ["bookmark", "list"] and len(args) > 2 and args[2] == "-T":
    for branch in load("cleanup-stack-bookmarks.json", []):
        print(f"{branch}\t")
    sys.exit(0)
if args[:2] == ["bookmark", "list"] and "--revision" in args:
    rev = args[args.index("--revision") + 1]
    local_heads = load("local-bookmarks.json", {})
    for branch, commit in sorted(local_heads.items()):
        if commit == rev:
            print(f"{branch}\t")
    sys.exit(0)
if args[:3] == ["bookmark", "list", "--all-remotes"] and "-T" in args:
    branch = args[3]
    heads = load("branch-heads.json", {})
    if branch in heads:
        print("origin\ttracked\tok")
    sys.exit(0)
if args[:3] == ["bookmark", "list", "jj-stack/frozen/*"] and "-T" in args:
    sys.exit(0)
if args[:2] == ["bookmark", "set"] and "-r" in args:
    branch = args[2]
    commit = args[args.index("-r") + 1]
    heads = load("local-bookmarks.json", {})
    heads[branch] = commit
    save("local-bookmarks.json", heads)
    sys.exit(0)
if args[:2] == ["bookmark", "delete"]:
    heads = load("local-bookmarks.json", {})
    heads.pop(args[2], None)
    save("local-bookmarks.json", heads)
    sys.exit(0)
if args[:2] == ["bookmark", "forget"]:
    sys.exit(0)
if args[:2] == ["git", "push"] and "--bookmark" in args:
    branches = [args[i + 1] for i, arg in enumerate(args[:-1]) if arg == "--bookmark"]
    heads = load("branch-heads.json", {})
    local_heads = load("local-bookmarks.json", {})
    for branch in branches:
        if branch in local_heads:
            heads[branch] = local_heads[branch]
    save("branch-heads.json", heads)
    if load("auto-merge-trunk.json", None) in branches:
        (root / "trunk-pushed").write_text("true")
        scenario = load("scenario.json", "")
        if scenario not in {"verify-merge-slow"}:
            prs = load("prs.json", [])
            for pr in prs:
                if pr["state"].upper() == "OPEN" and pr["baseRefName"] == "main":
                    pr["state"] = "MERGED"
            save("prs.json", prs)
    sys.exit(0)
if args and args[0] == "new":
    sys.exit(0)

print("unconfigured jj command: " + " ".join(args), file=sys.stderr)
sys.exit(1)
"#;

const GIT_FAKE: &str = r#"#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

root = Path(os.environ["STACK_FAKE_ROOT"])
args = sys.argv[1:]
with (root / "git-requests.jsonl").open("a") as fh:
    fh.write(json.dumps({"args": args}) + "\n")

def load(name, default):
    path = root / name
    return json.loads(path.read_text()) if path.exists() else default

if args[:2] == ["config", "--get"]:
    sys.exit(1)
if args[:3] == ["show", "-s", "--format=%T"]:
    print("tree-" + args[3][:12])
    sys.exit(0)
if args[:2] == ["merge-base", "--is-ancestor"]:
    ancestors = load("ancestors.json", {})
    if ancestors.get(args[2] + "\n" + args[3], True):
        sys.exit(0)
    print("not ancestor", file=sys.stderr)
    sys.exit(1)
if args and args[0] == "rev-parse":
    refs = load("rev-parse.json", {})
    value = refs.get(args[1])
    if value is None:
        print("missing rev-parse", file=sys.stderr)
        sys.exit(1)
    print(value)
    sys.exit(0)

print("unconfigured git command: " + " ".join(args), file=sys.stderr)
sys.exit(1)
"#;

const GH_FAKE: &str = r#"#!/usr/bin/env python3
import json
import os
import sys
import time
from pathlib import Path

root = Path(os.environ["STACK_FAKE_ROOT"])
args = sys.argv[1:]
with (root / "gh-requests.jsonl").open("a") as fh:
    fh.write(json.dumps({"args": args}) + "\n")

def load(name, default):
    path = root / name
    return json.loads(path.read_text()) if path.exists() else default

def save(name, value):
    (root / name).write_text(json.dumps(value))

def counter(name):
    path = root / name
    value = int(path.read_text()) if path.exists() else 0
    value += 1
    path.write_text(str(value))
    return value

def field_values():
    values = {}
    for index, arg in enumerate(args):
        if arg == "-f" and index + 1 < len(args):
            key, _, value = args[index + 1].partition("=")
            values[key] = value
    return values

def pr_response(number, state, head, base, title, body):
    heads = load("branch-heads.json", {})
    return {
        "number": number,
        "state": state,
        "id": f"PR_node_{number}",
        "headRefName": head,
        "baseRefName": base,
        "headRefOid": heads.get(head, "headsha"),
        "baseRefOid": heads.get(base, "basesha"),
        "headRepository": {"id": "repo-id", "node_id": "repo-node", "nameWithOwner": "owner/repo"},
        "baseRepository": {"id": "repo-id", "node_id": "repo-node", "nameWithOwner": "owner/repo"},
        "author": {"login": "octocat"},
        "title": title,
        "body": body,
        "createdAt": "2026-06-03T12:34:56Z",
        "isDraft": False,
        "reviewDecision": "APPROVED",
        "mergeable": "MERGEABLE",
        "mergeStateStatus": "CLEAN",
        "statusCheckRollup": [{"context": "ci", "state": "SUCCESS"}],
        "autoMergeRequest": None,
    }

if args[:2] == ["repo", "view"]:
    print("owner/repo")
    sys.exit(0)
if args[:3] == ["api", "user", "--jq"]:
    print("octocat")
    sys.exit(0)
if args[:2] == ["pr", "list"]:
    state = args[args.index("--state") + 1].upper()
    prs = [pr for pr in load("prs.json", []) if pr["state"].upper() == state]
    if "--head" in args:
        head = args[args.index("--head") + 1]
        prs = [pr for pr in prs if pr["headRefName"] == head]
    if "--base" in args:
        base = args[args.index("--base") + 1]
        prs = [pr for pr in prs if pr["baseRefName"] == base]
    print(json.dumps(prs))
    sys.exit(0)
if args[:2] == ["pr", "view"]:
    number = int(args[2])
    jq = args[args.index("--jq") + 1] if "--jq" in args else None
    prs = load("prs.json", [])
    scenario = load("scenario.json", "")
    for pr in prs:
        if int(pr["number"]) != number:
            continue
        if jq == ".state":
            if scenario == "verify-merge-slow" and (root / "trunk-pushed").exists():
                time.sleep(0.35)
                if counter("verify-state-count") >= 4:
                    for item in prs:
                        if item["baseRefName"] == "main":
                            item["state"] = "MERGED"
                    save("prs.json", prs)
                    pr["state"] = "MERGED"
            print(pr["state"])
            sys.exit(0)
        if scenario == "merge-two-prs-settle-slow" and number == 12 and pr.get("mergeable") == "UNKNOWN":
            time.sleep(0.35)
            if counter("mergeability-12-count") >= 3:
                pr["mergeable"] = "MERGEABLE"
                save("prs.json", prs)
        print(json.dumps(pr))
        sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)
if args[:1] == ["api"] and len(args) >= 2 and args[1].startswith("repos/owner/repo/pulls/") and "-X" not in args:
    number = int(args[1].rsplit("/", 1)[1])
    for pr in load("prs.json", []):
        if int(pr["number"]) == number:
            print(json.dumps(pr))
            sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)
if args[:3] == ["api", "-X", "PATCH"] and args[3].startswith("repos/owner/repo/pulls/"):
    number = int(args[3].rsplit("/", 1)[1])
    values = field_values()
    prs = load("prs.json", [])
    scenario = load("scenario.json", "")
    for pr in prs:
        if int(pr["number"]) == number:
            base = values.get("base", pr["baseRefName"])
            pr.update(pr_response(number, pr["state"], pr["headRefName"], base, pr.get("title", ""), pr.get("body", "")))
            if scenario == "merge-two-prs-settle-slow" and number == 12:
                pr["mergeable"] = "UNKNOWN"
            print(json.dumps(pr))
            save("prs.json", prs)
            sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)

print("unconfigured gh command: " + " ".join(args), file=sys.stderr)
sys.exit(1)
"#;
