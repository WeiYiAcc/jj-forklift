use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::Deserialize;

struct RealGithubRepo {
    root: PathBuf,
    work: PathBuf,
    full_name: String,
    keep: bool,
    deleted: bool,
}

#[derive(Clone, Debug)]
struct TestChange {
    change_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct GithubPr {
    number: u64,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(rename = "baseRefName")]
    base_ref_name: String,
    title: String,
}

impl RealGithubRepo {
    fn create() -> anyhow::Result<Self> {
        let owner = required_env("FORKLIFT_E2E_OWNER")?;
        let prefix = required_env("FORKLIFT_E2E_REPO_PREFIX")?;
        let root = unique_dir("real-github")?;
        let work = root.join("work");
        let suffix = unique_suffix()?;
        let repo_name = format!("{prefix}-{suffix}");
        let full_name = format!("{owner}/{repo_name}");

        run_ok(
            "gh",
            &[
                "repo",
                "create",
                full_name.as_str(),
                "--private",
                "--disable-issues",
                "--disable-wiki",
                "--description",
                "Disposable Forklift E2E repository",
            ],
        )
        .with_context(|| {
            format!(
                "create GitHub repo `{full_name}`; ensure `gh auth status` is valid and the owner allows repo creation"
            )
        })?;

        run_ok_in(&root, "jj", &["git", "init", "--colocate", "work"])?;
        run_ok_in(&work, "git", &["config", "user.email", "test@example.com"])?;
        run_ok_in(&work, "git", &["config", "user.name", "Forklift E2E"])?;
        run_ok_in(
            &work,
            "jj",
            &["config", "set", "--repo", "user.email", "test@example.com"],
        )?;
        run_ok_in(
            &work,
            "jj",
            &["config", "set", "--repo", "user.name", "Forklift E2E"],
        )?;
        run_ok_in(
            &work,
            "jj",
            &["config", "set", "--repo", "stack.require-approval", "false"],
        )?;
        run_ok_in(
            &work,
            "git",
            &[
                "remote",
                "add",
                "origin",
                &format!("https://github.com/{full_name}.git"),
            ],
        )?;

        Ok(Self {
            root,
            work,
            full_name,
            keep: env_bool("FORKLIFT_E2E_KEEP_REPO"),
            deleted: false,
        })
    }

    fn init_main(&self) -> anyhow::Result<()> {
        fs::write(self.work.join("README.md"), "# forklift e2e\n")?;
        run_ok_in(&self.work, "jj", &["describe", "-m", "initial"])?;
        run_ok_in(&self.work, "jj", &["bookmark", "set", "main", "-r", "@"])?;
        run_ok_in(
            &self.work,
            "jj",
            &["git", "push", "--remote", "origin", "--bookmark", "main"],
        )
    }

    fn create_change(&self, name: &str, title: &str) -> anyhow::Result<TestChange> {
        if !self.current_change_is_empty_undescribed()? {
            run_ok_in(&self.work, "jj", &["new"])?;
        }
        fs::write(self.work.join(format!("{name}.txt")), format!("{title}\n"))?;
        run_ok_in(&self.work, "jj", &["describe", "-m", title])?;
        Ok(TestChange {
            change_id: run_stdout_in(
                &self.work,
                "jj",
                &["log", "--no-graph", "-r", "@", "-T", "change_id"],
            )?
            .trim()
            .to_owned(),
        })
    }

    fn edit_change(&self, change: &TestChange, name: &str, title: &str) -> anyhow::Result<()> {
        run_ok_in(&self.work, "jj", &["edit", &change.change_id])?;
        fs::write(self.work.join(format!("{name}.txt")), format!("{title}\n"))?;
        run_ok_in(&self.work, "jj", &["describe", "-m", title])
    }

    fn edit_top(&self, change: &TestChange) -> anyhow::Result<()> {
        run_ok_in(&self.work, "jj", &["edit", &change.change_id])
    }

    fn current_change_is_empty_undescribed(&self) -> anyhow::Result<bool> {
        let output = run_stdout_in(
            &self.work,
            "jj",
            &[
                "log",
                "--no-graph",
                "-r",
                "@",
                "-T",
                "empty ++ \"\\n\" ++ description",
            ],
        )?;
        let mut lines = output.lines();
        Ok(lines.next() == Some("true") && lines.all(|line| line.is_empty()))
    }

    fn run_forklift(&self, args: &[&str]) -> anyhow::Result<Output> {
        Ok(Command::new(env!("CARGO_BIN_EXE_forklift"))
            .args(args)
            .current_dir(&self.work)
            .output()?)
    }

    fn open_prs(&self) -> anyhow::Result<Vec<GithubPr>> {
        let stdout = run_stdout(
            "gh",
            &[
                "pr",
                "list",
                "--repo",
                self.full_name.as_str(),
                "--state",
                "open",
                "--json",
                "number,headRefName,baseRefName,title",
                "--limit",
                "20",
            ],
        )?;
        let mut prs = serde_json::from_str::<Vec<GithubPr>>(&stdout)
            .with_context(|| format!("parse PR list for {}", self.full_name))?;
        prs.sort_by_key(|pr| pr.number);
        Ok(prs)
    }

    fn wait_for_open_prs(&self, expected: usize) -> anyhow::Result<Vec<GithubPr>> {
        let mut last = Vec::new();
        for _ in 0..12 {
            last = self.open_prs()?;
            if last.len() == expected {
                return Ok(last);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        bail!(
            "expected {expected} open PRs in {}, got {}: {last:#?}",
            self.full_name,
            last.len()
        )
    }

    fn cleanup(&mut self) -> anyhow::Result<()> {
        if self.keep || self.deleted {
            return Ok(());
        }
        run_ok("gh", &["repo", "delete", self.full_name.as_str(), "--yes"]).with_context(|| {
            format!(
                "delete GitHub repo `{}`; run `gh auth refresh -s delete_repo` if cleanup lacks permission",
                self.full_name
            )
        })?;
        self.deleted = true;
        Ok(())
    }
}

impl Drop for RealGithubRepo {
    fn drop(&mut self) {
        if !self.keep && !self.deleted {
            let _ = Command::new("gh")
                .args(["repo", "delete", self.full_name.as_str(), "--yes"])
                .output();
        }
        if !self.keep {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn real_github_submit_update_and_optional_merge() -> anyhow::Result<()> {
    if !env_bool("FORKLIFT_REAL_GITHUB_E2E") {
        eprintln!("skipped real GitHub E2E; set FORKLIFT_REAL_GITHUB_E2E=1 to run it");
        return Ok(());
    }

    let mut repo = RealGithubRepo::create()?;
    repo.init_main()?;
    let bottom = repo.create_change("bottom", "bottom title")?;
    let top = repo.create_change("top", "top title")?;

    let output = repo.run_forklift(&["submit", "--yes"])?;
    assert_success("initial forklift submit", &output);

    let initial = repo.wait_for_open_prs(2)?;
    let bottom_pr = pr_with_title(&initial, "bottom title")?;
    let top_pr = pr_with_title(&initial, "top title")?;
    assert_eq!(bottom_pr.base_ref_name, "main");
    assert_eq!(top_pr.base_ref_name, bottom_pr.head_ref_name);

    repo.edit_change(&bottom, "bottom", "bottom title edited")?;
    repo.edit_top(&top)?;
    let output = repo.run_forklift(&["submit", "--yes"])?;
    assert_success("updated forklift submit", &output);

    let updated = repo.wait_for_open_prs(2)?;
    let edited_bottom = pr_with_title(&updated, "bottom title edited")?;
    let updated_top = pr_with_title(&updated, "top title")?;
    assert_eq!(edited_bottom.number, bottom_pr.number);
    assert_eq!(edited_bottom.head_ref_name, bottom_pr.head_ref_name);
    assert_eq!(updated_top.number, top_pr.number);
    assert_eq!(updated_top.head_ref_name, top_pr.head_ref_name);
    assert_eq!(updated_top.base_ref_name, edited_bottom.head_ref_name);

    let merge = repo.run_forklift(&["merge", "--verbose"])?;
    if merge.status.success() {
        let remaining = repo.wait_for_open_prs(0)?;
        assert!(
            remaining.is_empty(),
            "successful merge should close all PRs: {remaining:#?}"
        );
    } else {
        eprintln!(
            "real GitHub merge skipped after submit/update verification; stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&merge.stdout),
            String::from_utf8_lossy(&merge.stderr)
        );
    }

    repo.cleanup()?;
    Ok(())
}

fn pr_with_title<'a>(prs: &'a [GithubPr], title: &str) -> anyhow::Result<&'a GithubPr> {
    prs.iter()
        .find(|pr| pr.title == title)
        .with_context(|| format!("missing PR titled `{title}` in {prs:#?}"))
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).with_context(|| format!("{name} is required when FORKLIFT_REAL_GITHUB_E2E=1"))
}

fn env_bool(name: &str) -> bool {
    matches!(
        env::var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn unique_dir(name: &str) -> anyhow::Result<PathBuf> {
    let path = env::temp_dir().join(format!("forklift-{name}-{}", unique_suffix()?));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn unique_suffix() -> anyhow::Result<String> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(format!("{}-{nanos}", std::process::id()))
}

fn run_ok(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new(program).args(args).output()?;
    assert_success(&display_command(program, args), &output);
    Ok(())
}

fn run_ok_in(dir: &Path, program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new(program).args(args).current_dir(dir).output()?;
    assert_success(&display_command(program, args), &output);
    Ok(())
}

fn run_stdout(program: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(program).args(args).output()?;
    assert_success(&display_command(program, args), &output);
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_stdout_in(dir: &Path, program: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(program).args(args).current_dir(dir).output()?;
    assert_success(&display_command(program, args), &output);
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn assert_success(command: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{command} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn display_command(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}
