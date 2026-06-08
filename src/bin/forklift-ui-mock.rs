use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

const CONFIG_PREFIX: &str = "stack";

fn main() -> Result<()> {
    let Some(scenario) = env::args().nth(1) else {
        print_scenarios();
        return Ok(());
    };
    match scenario.as_str() {
        "submit-actions" => run_submit_actions_scenario(),
        "sync-only" => run_sync_only_scenario(),
        "sync-submit" => run_sync_submit_scenario(),
        "merge-two-prs-settle-slow" => run_merge_scenario(Scenario::SettleSlow),
        "verify-merge-slow" => run_merge_scenario(Scenario::VerifySlow),
        "cleanup-branches" => run_merge_scenario(Scenario::CleanupBranches),
        other => {
            bail!(
                "unknown scenario `{other}`\nknown scenarios:\n{}",
                scenario_list().join("\n")
            )
        }
    }
}

fn print_scenarios() {
    for scenario in scenario_list() {
        println!("{scenario}");
    }
}

fn scenario_list() -> &'static [&'static str] {
    &[
        "submit-actions",
        "sync-only",
        "sync-submit",
        "merge-two-prs-settle-slow",
        "verify-merge-slow",
        "cleanup-branches",
    ]
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

fn run_submit_actions_scenario() -> Result<()> {
    let fixture = Fixture::new("submit-actions")?;
    fixture.seed_submit_actions()?;

    eprintln!("forklift-ui-mock: submit-actions");
    eprintln!("fixture: {}", fixture.root.display());
    eprintln!("running: forklift submit --yes");

    let status = fixture.run_forklift(["submit", "--yes"])?;
    if !status.success() {
        bail!("scenario `submit-actions` failed with status {status}");
    }

    eprintln!(
        "scenario complete; fixture kept at {}",
        fixture.root.display()
    );
    Ok(())
}

fn run_sync_only_scenario() -> Result<()> {
    let fixture = Fixture::new("sync-only")?;
    fixture.seed_sync_stack()?;

    eprintln!("forklift-ui-mock: sync-only");
    eprintln!("fixture: {}", fixture.root.display());
    eprintln!("running: forklift sync");

    let status = fixture.run_forklift(["sync"])?;
    if !status.success() {
        bail!("scenario `sync-only` failed with status {status}");
    }

    eprintln!(
        "scenario complete; fixture kept at {}",
        fixture.root.display()
    );
    Ok(())
}

fn run_sync_submit_scenario() -> Result<()> {
    let fixture = Fixture::new("sync-submit")?;
    fixture.seed_submit_actions()?;

    eprintln!("forklift-ui-mock: sync-submit");
    eprintln!("fixture: {}", fixture.root.display());
    eprintln!("running: forklift sync --submit --yes");

    let status = fixture.run_forklift(["sync", "--submit", "--yes"])?;
    if !status.success() {
        bail!("scenario `sync-submit` failed with status {status}");
    }

    eprintln!(
        "scenario complete; fixture kept at {}",
        fixture.root.display()
    );
    Ok(())
}

fn run_merge_scenario(scenario: Scenario) -> Result<()> {
    let fixture = Fixture::new(scenario.name())?;
    fixture.seed_two_pr_merge(scenario)?;

    eprintln!("forklift-ui-mock: {}", scenario.name());
    eprintln!("fixture: {}", fixture.root.display());
    eprintln!("running: forklift merge --admin");

    let status = fixture.run_forklift(["merge", "--admin"])?;
    if !status.success() {
        bail!("scenario `{}` failed with status {status}", scenario.name());
    }

    eprintln!(
        "scenario complete; fixture kept at {}",
        fixture.root.display()
    );
    Ok(())
}

#[derive(Clone, Debug)]
struct Change {
    commit_id: String,
    change_id: String,
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
        let remote = root.join("remote.git");

        fs::create_dir_all(&bin_dir)?;
        run_ok_in(&root, "jj", &["git", "init", "--colocate", "workspace"])?;
        run_ok("git", &["init", "--bare", remote.to_str().unwrap()])?;
        run_ok_in(
            &workspace,
            "git",
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )?;
        run_ok_in(
            &workspace,
            "git",
            &["config", "user.email", "test@example.com"],
        )?;
        run_ok_in(&workspace, "git", &["config", "user.name", "Test User"])?;
        run_ok_in(
            &workspace,
            "jj",
            &["config", "set", "--repo", "user.email", "test@example.com"],
        )?;
        run_ok_in(
            &workspace,
            "jj",
            &["config", "set", "--repo", "user.name", "Test User"],
        )?;

        let fixture = Self {
            root,
            workspace,
            repo_dir,
            bin_dir,
        };
        fixture.install_fake_gh()?;
        fixture.write_gh_state(&json!({ "prs": [], "comments": {} }))?;
        Ok(fixture)
    }

    fn install_fake_gh(&self) -> Result<()> {
        write_executable(&self.bin_dir.join("gh"), GH_FAKE)
    }

    fn seed_submit_actions(&self) -> Result<()> {
        let main = self.init_main()?;
        let bottom = self.create_change("bottom.txt", "bottom title", "bottom body", "main")?;
        let middle = self.create_change("middle.txt", "middle title", "middle body", "@")?;
        let _top = self.create_change("top.txt", "top title", "top body", "@")?;
        let bottom_branch = stack_branch("bottom-title", &bottom.change_id);
        let middle_branch = stack_branch("middle-title", &middle.change_id);

        self.set_bookmark(&bottom_branch, &bottom.commit_id)?;
        self.set_bookmark(&middle_branch, &middle.commit_id)?;
        self.push_bookmark(&bottom_branch)?;
        self.push_bookmark(&middle_branch)?;

        self.write_gh_state(&json!({
            "next_pr_number": 13,
            "prs": [
                pr_json(11, "OPEN", &bottom_branch, "main", "bottom title", "bottom body"),
                pr_json(12, "OPEN", &middle_branch, &bottom_branch, "middle title before update", "middle body before update"),
            ],
            "comments": {},
        }))?;
        self.write_cache(
            &bottom.change_id,
            11,
            &bottom_branch,
            "main",
            &bottom.commit_id,
            &main.commit_id,
            "bottom title",
            "bottom body",
        )?;
        self.write_cache(
            &middle.change_id,
            12,
            &middle_branch,
            &bottom_branch,
            &middle.commit_id,
            &bottom.commit_id,
            "middle title before update",
            "middle body before update",
        )?;
        Ok(())
    }

    fn seed_sync_stack(&self) -> Result<()> {
        self.init_main()?;
        let local = self.create_change("local.txt", "local title", "local body", "main")?;
        self.create_change("upstream.txt", "upstream title", "upstream body", "main")?;
        self.set_bookmark("main", "@")?;
        self.push_bookmark("main")?;
        run_ok_in(&self.workspace, "jj", &["edit", &local.change_id])?;
        Ok(())
    }

    fn seed_two_pr_merge(&self, scenario: Scenario) -> Result<()> {
        let main = self.init_main()?;
        let bottom = self.create_change("bottom.txt", "bottom title", "bottom body", "main")?;
        let top = self.create_change("top.txt", "top title", "top body", "@")?;
        let bottom_branch = stack_branch("bottom-title", &bottom.change_id);
        let top_branch = stack_branch("top-title", &top.change_id);

        self.set_bookmark(&bottom_branch, &bottom.commit_id)?;
        self.set_bookmark(&top_branch, &top.commit_id)?;
        self.push_bookmark(&bottom_branch)?;
        self.push_bookmark(&top_branch)?;

        self.write_gh_state(&json!({
            "prs": [
                pr_json(11, "OPEN", &bottom_branch, "main", "bottom title", "bottom body"),
                pr_json(12, "OPEN", &top_branch, &bottom_branch, "top title", "top body"),
            ],
            "comments": {},
        }))?;
        self.write_cache(
            &bottom.change_id,
            11,
            &bottom_branch,
            "main",
            &bottom.commit_id,
            &main.commit_id,
            "bottom title",
            "bottom body",
        )?;
        self.write_cache(
            &top.change_id,
            12,
            &top_branch,
            &bottom_branch,
            &top.commit_id,
            &bottom.commit_id,
            "top title",
            "top body",
        )?;

        fs::write(
            self.root.join("scenario.json"),
            serde_json::to_string(scenario.name())?,
        )?;
        Ok(())
    }

    fn init_main(&self) -> Result<Change> {
        fs::write(self.workspace.join("file.txt"), "main\n")?;
        run_ok_in(&self.workspace, "jj", &["describe", "-m", "initial"])?;
        run_ok_in(
            &self.workspace,
            "jj",
            &["bookmark", "set", "main", "-r", "@"],
        )?;
        let main = self.change_at("@")?;
        self.push_bookmark("main")?;
        Ok(main)
    }

    fn create_change(
        &self,
        filename: &str,
        title: &str,
        body: &str,
        parent: &str,
    ) -> Result<Change> {
        run_ok_in(&self.workspace, "jj", &["new", parent])?;
        fs::write(self.workspace.join(filename), format!("{title}\n{body}\n"))?;
        run_ok_in(
            &self.workspace,
            "jj",
            &["describe", "-m", title, "-m", body],
        )?;
        self.change_at("@")
    }

    fn set_bookmark(&self, name: &str, rev: &str) -> Result<()> {
        run_ok_in(&self.workspace, "jj", &["bookmark", "set", name, "-r", rev])
    }

    fn push_bookmark(&self, name: &str) -> Result<()> {
        run_ok_in(
            &self.workspace,
            "jj",
            &["git", "push", "--remote", "origin", "--bookmark", name],
        )
    }

    fn change_at(&self, rev: &str) -> Result<Change> {
        Ok(Change {
            commit_id: self.rev_output(rev, "commit_id")?,
            change_id: self.rev_output(rev, "change_id")?,
        })
    }

    fn rev_output(&self, rev: &str, template: &str) -> Result<String> {
        let output = command_output_in(
            &self.workspace,
            "jj",
            &["log", "--no-graph", "-r", rev, "-T", template],
        )?;
        Ok(output.trim().to_owned())
    }

    fn run_forklift<const N: usize>(&self, args: [&str; N]) -> Result<std::process::ExitStatus> {
        let old_path = env::var("PATH").unwrap_or_default();
        let mut command = Command::new(resolve_forklift_bin()?);
        command
            .args(args)
            .current_dir(&self.workspace)
            .env("PATH", format!("{}:{old_path}", self.bin_dir.display()))
            .env("FORKLIFT_UI_MOCK_ROOT", &self.root)
            .env("FORKLIFT_UI_MOCK_WORKSPACE", &self.workspace)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        command.status().map_err(Into::into)
    }

    fn gh_state_path(&self) -> PathBuf {
        self.root.join("gh-state.json")
    }

    fn write_gh_state(&self, state: &Value) -> Result<()> {
        fs::write(self.gh_state_path(), serde_json::to_string(state)?).map_err(Into::into)
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

fn resolve_forklift_bin() -> Result<PathBuf> {
    if let Some(path) = env::var_os("FORKLIFT_BIN") {
        let path = PathBuf::from(path);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Ok(env::current_dir()?.join(path))
        };
    }
    let current = env::current_exe()?;
    if let Some(dir) = current.parent() {
        let sibling = dir.join("forklift");
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    Ok(PathBuf::from("forklift"))
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

fn run_ok(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program).args(args).output()?;
    assert_success(&display_command(program, args), &output)
}

fn run_ok_in(dir: &Path, program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program).args(args).current_dir(dir).output()?;
    assert_success(&display_command(program, args), &output)
}

fn command_output_in(dir: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).current_dir(dir).output()?;
    assert_success(&display_command(program, args), &output)?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn assert_success(label: &str, output: &std::process::Output) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "{label} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

fn display_command(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
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
    let path = env::temp_dir().join(format!(
        "forklift-ui-mock-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn stack_branch(title_slug: &str, change_id: &str) -> String {
    format!("stack/{title_slug}-{}", &change_id[..8])
}

fn pr_json(number: u64, state: &str, head: &str, base: &str, title: &str, body: &str) -> Value {
    json!({
        "number": number,
        "state": state,
        "headRefName": head,
        "baseRefName": base,
        "title": title,
        "body": body,
    })
}

const GH_FAKE: &str = r#"#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import time
from pathlib import Path

root = Path(os.environ["FORKLIFT_UI_MOCK_ROOT"])
workspace = Path(os.environ["FORKLIFT_UI_MOCK_WORKSPACE"])
args = sys.argv[1:]

with (root / "gh-requests.jsonl").open("a") as fh:
    fh.write(json.dumps({"args": args}) + "\n")

def load(name, default):
    path = root / name
    return json.loads(path.read_text()) if path.exists() else default

def save(name, value):
    (root / name).write_text(json.dumps(value))

def load_state():
    state = load("gh-state.json", {"prs": [], "comments": {}})
    state.setdefault("prs", [])
    state.setdefault("comments", {})
    return state

def save_state(state):
    save("gh-state.json", state)

def counter(name):
    path = root / name
    value = int(path.read_text()) if path.exists() else 0
    value += 1
    path.write_text(str(value))
    return value

def git_out(*git_args):
    try:
        return subprocess.check_output(
            ["git", "-C", str(workspace), *git_args],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except subprocess.CalledProcessError:
        return ""

def remote_oid(branch):
    out = git_out("ls-remote", "origin", "refs/heads/" + branch)
    return out.split()[0] if out else ""

def is_ancestor(left, right):
    if not left or not right:
        return False
    return subprocess.call(
        ["git", "-C", str(workspace), "merge-base", "--is-ancestor", left, right],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ) == 0

def field_values():
    values = {}
    for index, arg in enumerate(args):
        if arg == "-f" and index + 1 < len(args):
            key, _, value = args[index + 1].partition("=")
            values[key] = value
    return values

def find_pr(state, number):
    for pr in state["prs"]:
        if int(pr["number"]) == int(number):
            return pr
    return None

def eligible_merged(pr):
    return is_ancestor(remote_oid(pr["headRefName"]), remote_oid(pr["baseRefName"]))

def resolve_state(state, pr, allow_slow_transition=False):
    if pr["state"].upper() != "OPEN":
        return pr["state"]
    if not eligible_merged(pr):
        return pr["state"]
    scenario = load("scenario.json", "")
    if scenario == "verify-merge-slow" and not allow_slow_transition:
        return pr["state"]
    if scenario == "verify-merge-slow" and counter("verify-state-count") < 4:
        time.sleep(0.35)
        return pr["state"]
    pr["state"] = "MERGED"
    save_state(state)
    return pr["state"]

def pr_view(state, pr, allow_slow_transition=False):
    scenario = load("scenario.json", "")
    mergeable = pr.get("mergeable", "MERGEABLE")
    if scenario == "merge-two-prs-settle-slow" and int(pr["number"]) == 12 and mergeable == "UNKNOWN":
        time.sleep(0.35)
        if counter("mergeability-12-count") >= 3:
            pr["mergeable"] = "MERGEABLE"
            mergeable = "MERGEABLE"
            save_state(state)
    return {
        "number": pr["number"],
        "state": resolve_state(state, pr, allow_slow_transition),
        "id": "PR_node_%d" % pr["number"],
        "headRefName": pr["headRefName"],
        "baseRefName": pr["baseRefName"],
        "headRefOid": remote_oid(pr["headRefName"]) or "headsha",
        "baseRefOid": remote_oid(pr["baseRefName"]) or "basesha",
        "headRepository": {"id": "repo-id", "node_id": "repo-node", "nameWithOwner": "owner/repo"},
        "baseRepository": {"id": "repo-id", "node_id": "repo-node", "nameWithOwner": "owner/repo"},
        "author": {"login": "octocat"},
        "title": pr.get("title", ""),
        "body": pr.get("body", ""),
        "createdAt": "2026-06-03T12:34:56Z",
        "isDraft": False,
        "reviewDecision": "APPROVED",
        "mergeable": mergeable,
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

if args[:2] == ["api", "repos/owner/repo"] and "--jq" in args:
    print("true")
    sys.exit(0)

if args[:2] == ["pr", "list"]:
    state = load_state()
    wanted = args[args.index("--state") + 1].upper() if "--state" in args else None
    head = args[args.index("--head") + 1] if "--head" in args else None
    base = args[args.index("--base") + 1] if "--base" in args else None
    prs = []
    for pr in state["prs"]:
        view = pr_view(state, pr)
        if wanted and view["state"].upper() != wanted:
            continue
        if head and view["headRefName"] != head:
            continue
        if base and view["baseRefName"] != base:
            continue
        prs.append(view)
    print(json.dumps(prs))
    sys.exit(0)

if args[:2] == ["pr", "view"]:
    state = load_state()
    pr = find_pr(state, args[2])
    if pr is None:
        print("not found", file=sys.stderr)
        sys.exit(1)
    jq = args[args.index("--jq") + 1] if "--jq" in args else None
    view = pr_view(state, pr, allow_slow_transition=(jq == ".state"))
    if jq == ".state":
        print(view["state"])
    else:
        print(json.dumps(view))
    sys.exit(0)

if args[:2] == ["pr", "merge"]:
    state = load_state()
    pr = find_pr(state, args[2])
    if pr is not None:
        pr["state"] = "MERGED"
        save_state(state)
    sys.exit(0)

if args[:1] == ["api"] and len(args) >= 2 and args[1].startswith("repos/owner/repo/pulls/") and "-X" not in args:
    state = load_state()
    pr = find_pr(state, args[1].rsplit("/", 1)[1])
    if pr is None:
        print("not found", file=sys.stderr)
        sys.exit(1)
    print(json.dumps(pr_view(state, pr)))
    sys.exit(0)

if args[:2] == ["api", "--paginate"] and "/issues/" in args[2] and args[2].endswith("/comments"):
    state = load_state()
    pr_number = args[2].split("/issues/")[1].split("/")[0]
    for comment in state["comments"].get(pr_number, []):
        print(json.dumps(comment))
    sys.exit(0)

if args[:3] == ["api", "-X", "PATCH"] and args[3].startswith("repos/owner/repo/pulls/"):
    state = load_state()
    pr = find_pr(state, args[3].rsplit("/", 1)[1])
    if pr is None:
        print("not found", file=sys.stderr)
        sys.exit(1)
    values = field_values()
    pr["baseRefName"] = values.get("base", pr["baseRefName"])
    pr["title"] = values.get("title", pr.get("title", ""))
    pr["body"] = values.get("body", pr.get("body", ""))
    scenario = load("scenario.json", "")
    if scenario == "merge-two-prs-settle-slow" and int(pr["number"]) == 12:
        pr["mergeable"] = "UNKNOWN"
    save_state(state)
    print(json.dumps(pr_view(state, pr)))
    sys.exit(0)

if args[:4] == ["api", "-X", "POST", "repos/owner/repo/pulls"]:
    state = load_state()
    values = field_values()
    number = int(state.get("next_pr_number") or (max([int(pr["number"]) for pr in state["prs"]] or [0]) + 1))
    state["next_pr_number"] = number + 1
    pr = {
        "number": number,
        "state": "OPEN",
        "headRefName": values.get("head", ""),
        "baseRefName": values.get("base", ""),
        "title": values.get("title", ""),
        "body": values.get("body", ""),
    }
    state["prs"].append(pr)
    save_state(state)
    print(json.dumps(pr_view(state, pr)))
    sys.exit(0)

if args[:3] == ["api", "-X", "POST"] and "/issues/" in args[3] and args[3].endswith("/comments"):
    state = load_state()
    pr_number = args[3].split("/issues/")[1].split("/")[0]
    values = field_values()
    existing = sum(len(items) for items in state["comments"].values())
    next_id = 100 + existing + 1
    state["comments"].setdefault(pr_number, []).append({
        "id": next_id,
        "body": values.get("body", ""),
        "userLogin": "octocat",
        "updatedAt": "2026-06-03T18:00:00Z",
    })
    save_state(state)
    print(json.dumps({"id": next_id}))
    sys.exit(0)

if args[:3] == ["api", "-X", "PATCH"] and "/issues/comments/" in args[3]:
    state = load_state()
    comment_id = int(args[3].rsplit("/", 1)[1])
    values = field_values()
    for items in state["comments"].values():
        for comment in items:
            if int(comment["id"]) == comment_id:
                comment["body"] = values.get("body", "")
                comment["updatedAt"] = "2026-06-03T18:30:00Z"
                save_state(state)
                print(json.dumps({"id": comment_id}))
                sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)

print("unconfigured gh command: " + " ".join(args), file=sys.stderr)
sys.exit(1)
"#;
