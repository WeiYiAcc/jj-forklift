use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde_json::{Value, json};

const CONFIG_PREFIX: &str = "stack";
const STACK_FIELD_SEPARATOR: char = '\x1f';
const STACK_RECORD_SEPARATOR: char = '\x1e';
const ROOT_PARENT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REMOTE_TRUNK: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const COMMIT_ONE: &str = "1111111111111111111111111111111111111111";
const COMMIT_TWO: &str = "2222222222222222222222222222222222222222";
const COMMIT_THREE: &str = "3333333333333333333333333333333333333333";
const COMMIT_FOUR: &str = "4444444444444444444444444444444444444444";

#[test]
fn integration_one_change_submit_creates_pr_with_fake_gh() -> anyhow::Result<()> {
    let fixture = Fixture::new("one-submit")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [(COMMIT_ONE, "main", ROOT_PARENT)],
        [],
    )?;
    fixture.write_branch_heads([("main", ROOT_PARENT)])?;
    fixture.write_pr_numbers([("stack/change-title-changeon", 9)])?;
    fixture.write_prs([])?;

    let output = fixture.run(["submit"])?;

    assert_success(&output);
    let jj = fixture.command_log("jj")?;
    assert!(
        jj.iter().any(|args| args
            == &[
                "git".to_owned(),
                "push".to_owned(),
                "--remote".to_owned(),
                "origin".to_owned(),
                "--bookmark".to_owned(),
                "stack/change-title-changeon".to_owned(),
            ]),
        "submit should push through jj bookmarks: {jj:#?}"
    );
    assert!(
        !fixture
            .command_log("git")?
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "push")),
        "submit should not raw-push through git"
    );
    let requests = fixture.gh_requests()?;
    assert!(contains_gh(
        &requests,
        &["api", "-X", "POST", "repos/owner/repo/pulls"]
    ));
    assert!(contains_gh_field(
        &requests,
        "head=stack/change-title-changeon"
    ));
    assert!(contains_gh_field(&requests, "base=main"));
    assert!(
        fixture.cache_path().exists(),
        "submit should save repo-private cache"
    );
    fixture.cleanup()
}

#[test]
fn integration_submit_dry_run_says_cache_writes_are_skipped() -> anyhow::Result<()> {
    let fixture = Fixture::new("submit-dry-run-cache-note")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [(COMMIT_ONE, "main", ROOT_PARENT)],
        [],
    )?;
    fixture.write_branch_heads([("main", ROOT_PARENT)])?;
    fixture.write_prs([])?;

    let output = fixture.run(["submit", "--dry-run"])?;

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("SQLite cache writes are skipped"),
        "{stdout}"
    );
    assert!(
        !fixture.cache_path().exists(),
        "dry-run submit must not create cache.sqlite"
    );
    let gh = fixture.gh_requests()?;
    assert!(contains_gh(&gh, &["pr", "list", "--repo", "owner/repo"]));
    assert!(!contains_gh(
        &gh,
        &["api", "-X", "POST", "repos/owner/repo/pulls"]
    ));
    fixture.cleanup()
}

#[test]
fn integration_two_change_submit_uses_parent_head_branch_base() -> anyhow::Result<()> {
    let fixture = Fixture::new("two-submit")?;
    fixture.write_jj_logs(&[stack_log(&[
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
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [
            (COMMIT_ONE, "main", ROOT_PARENT),
            (COMMIT_TWO, COMMIT_ONE, COMMIT_ONE),
        ],
        [],
    )?;
    fixture.write_branch_heads([("main", ROOT_PARENT)])?;
    fixture.write_pr_numbers([
        ("stack/bottom-title-bottomch", 11),
        ("stack/top-title-topchang", 12),
    ])?;
    fixture.write_prs([])?;

    let output = fixture.run(["submit"])?;

    assert_success(&output);
    let requests = fixture.gh_requests()?;
    let top_create = requests.iter().find(|request| {
        request_args(request).starts_with(&[
            "api".to_owned(),
            "-X".to_owned(),
            "POST".to_owned(),
            "repos/owner/repo/pulls".to_owned(),
        ]) && request_args(request).contains(&"head=stack/top-title-topchang".to_owned())
    });
    let Some(top_create) = top_create else {
        panic!("expected a create request for the top PR: {requests:#?}");
    };
    assert!(
        request_args(top_create).contains(&"base=stack/bottom-title-bottomch".to_owned()),
        "top PR should target the bottom PR branch: {top_create:#?}"
    );
    fixture.cleanup()
}

#[test]
fn integration_two_change_update_keeps_top_pr_based_on_bottom_branch() -> anyhow::Result<()> {
    let fixture = Fixture::new("two-update-bottom")?;
    let bottom_branch = "stack/bottom-title-bottomch";
    let top_branch = "stack/top-title-topchang";
    fixture.write_jj_logs(&[stack_log(&[
        change_record(
            "bottomchange",
            COMMIT_THREE,
            &[ROOT_PARENT],
            "bottom title edited",
            "bottom title edited\n\nbottom body edited",
        ),
        change_record(
            "topchange",
            COMMIT_FOUR,
            &[COMMIT_THREE],
            "top title",
            "top title\n\ntop body",
        ),
    ])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [
            (COMMIT_THREE, "main", ROOT_PARENT),
            (COMMIT_FOUR, COMMIT_THREE, COMMIT_THREE),
        ],
        [],
    )?;
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        (bottom_branch, COMMIT_ONE),
        (top_branch, COMMIT_TWO),
    ])?;
    fixture.write_local_bookmarks([
        ("main", ROOT_PARENT),
        (bottom_branch, COMMIT_THREE),
        (top_branch, COMMIT_FOUR),
    ])?;
    fixture.write_prs([
        pr_json(
            11,
            "OPEN",
            bottom_branch,
            "main",
            COMMIT_ONE,
            ROOT_PARENT,
            "bottom title",
            "bottom body",
        ),
        pr_json(
            12,
            "OPEN",
            top_branch,
            bottom_branch,
            COMMIT_TWO,
            COMMIT_ONE,
            "top title",
            "top body",
        ),
    ])?;
    fixture.write_cache(
        "bottomchange",
        11,
        bottom_branch,
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "bottom title",
        "bottom body",
    )?;
    fixture.write_cache(
        "topchange",
        12,
        top_branch,
        bottom_branch,
        COMMIT_TWO,
        COMMIT_ONE,
        "top title",
        "top body",
    )?;
    fixture.write_comments([
        (
            "11",
            vec![json!({
                "id": 111,
                "body": stack_comment_body(
                    &[
                        ("bottomchange", 11, bottom_branch, "main", "bottom title"),
                        ("topchange", 12, top_branch, bottom_branch, "top title"),
                    ],
                    "bottomchange",
                ),
                "userLogin": "octocat",
                "updatedAt": "2026-06-03T17:00:00Z",
            })],
        ),
        (
            "12",
            vec![json!({
                "id": 222,
                "body": stack_comment_body(
                    &[
                        ("bottomchange", 11, bottom_branch, "main", "bottom title"),
                        ("topchange", 12, top_branch, bottom_branch, "top title"),
                    ],
                    "topchange",
                ),
                "userLogin": "octocat",
                "updatedAt": "2026-06-03T17:00:00Z",
            })],
        ),
    ])?;

    let output = fixture.run(["submit"])?;

    assert_success(&output);
    let gh = fixture.gh_requests()?;
    let bottom_update = gh.iter().find(|request| {
        request_args(request).starts_with(&[
            "api".to_owned(),
            "-X".to_owned(),
            "PATCH".to_owned(),
            "repos/owner/repo/pulls/11".to_owned(),
        ])
    });
    let Some(bottom_update) = bottom_update else {
        panic!("expected bottom PR update: {gh:#?}");
    };
    assert!(
        request_args(bottom_update).contains(&"base=main".to_owned()),
        "bottom PR should still target trunk: {bottom_update:#?}"
    );
    assert!(
        request_args(bottom_update).contains(&"title=bottom title edited".to_owned()),
        "bottom PR title should update: {bottom_update:#?}"
    );
    let top_update = gh.iter().find(|request| {
        request_args(request).starts_with(&[
            "api".to_owned(),
            "-X".to_owned(),
            "PATCH".to_owned(),
            "repos/owner/repo/pulls/12".to_owned(),
        ])
    });
    let Some(top_update) = top_update else {
        panic!("expected top PR update: {gh:#?}");
    };
    assert!(
        request_args(top_update).contains(&format!("base={bottom_branch}")),
        "top PR should remain based on the bottom branch: {top_update:#?}"
    );

    let prs: Vec<Value> =
        serde_json::from_str(&fs::read_to_string(fixture.root.join("prs.json"))?)?;
    let bottom = prs
        .iter()
        .find(|pr| pr["number"] == 11)
        .unwrap_or_else(|| panic!("missing bottom PR"));
    let top = prs
        .iter()
        .find(|pr| pr["number"] == 12)
        .unwrap_or_else(|| panic!("missing top PR"));
    assert_eq!(bottom["headRefOid"], COMMIT_THREE);
    assert_eq!(top["headRefOid"], COMMIT_FOUR);
    assert_eq!(top["baseRefName"], bottom_branch);
    assert_eq!(top["baseRefOid"], COMMIT_THREE);
    fixture.cleanup()
}

#[test]
fn integration_get_fetches_stack_from_comment_and_writes_cache() -> anyhow::Result<()> {
    let fixture = Fixture::new("get-stack")?;
    fixture.write_jj_logs(&[stack_log(&[
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
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        ("stack/bottom-title-bottomch", COMMIT_ONE),
        ("stack/top-title-topchang", COMMIT_TWO),
    ])?;
    fixture.write_prs([
        pr_json(
            11,
            "OPEN",
            "stack/bottom-title-bottomch",
            "main",
            COMMIT_ONE,
            ROOT_PARENT,
            "bottom title",
            "bottom body",
        ),
        pr_json(
            12,
            "OPEN",
            "stack/top-title-topchang",
            "stack/bottom-title-bottomch",
            COMMIT_TWO,
            COMMIT_ONE,
            "top title",
            "top body",
        ),
    ])?;
    fixture.write_comments([(
        "12",
        vec![json!({
            "id": 201,
            "body": stack_comment_body(
                &[
                    ("bottomchange", 11, "stack/bottom-title-bottomch", "main", "bottom title"),
                    ("topchange", 12, "stack/top-title-topchang", "stack/bottom-title-bottomch", "top title"),
                ],
                "topchange",
            ),
            "userLogin": "octocat",
            "updatedAt": "2026-06-03T17:00:00Z",
        })],
    )])?;

    let output = fixture.run(["get", "12"])?;

    assert_success(&output);
    let jj = fixture.command_log("jj")?;
    assert!(
        jj.iter().any(|args| {
            args == &[
                "git".to_owned(),
                "fetch".to_owned(),
                "--remote".to_owned(),
                "origin".to_owned(),
                "--branch".to_owned(),
                "stack/bottom-title-bottomch".to_owned(),
                "--branch".to_owned(),
                "stack/top-title-topchang".to_owned(),
            ]
        }),
        "get should fetch every branch in stack order: {jj:#?}"
    );
    assert!(
        jj.iter().any(|args| args
            == &[
                "bookmark".to_owned(),
                "set".to_owned(),
                "jj-stack/frozen/pr-11".to_owned(),
                "-r".to_owned(),
                COMMIT_ONE.to_owned(),
            ]),
        "get should freeze the bottom PR head: {jj:#?}"
    );
    assert!(
        jj.iter().any(|args| args
            == &[
                "bookmark".to_owned(),
                "set".to_owned(),
                "jj-stack/frozen/pr-12".to_owned(),
                "-r".to_owned(),
                COMMIT_TWO.to_owned(),
            ]),
        "get should freeze the top PR head: {jj:#?}"
    );
    assert!(
        !jj.iter()
            .any(|args| args.first().is_some_and(|arg| arg == "edit")),
        "get should not edit imported frozen history automatically: {jj:#?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("next: `jj new jj-stack/frozen/pr-12`"),
        "get should print the next command: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(fixture.cache_entry("bottomchange")?["pr_number"], json!(11));
    assert_eq!(fixture.cache_entry("topchange")?["pr_number"], json!(12));
    fixture.cleanup()
}

#[test]
fn integration_get_imports_single_pr_without_stack_comment() -> anyhow::Result<()> {
    let fixture = Fixture::new("get-single-pr")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "singlechange",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "single title",
        "single title\n\nsingle body",
    )])])?;
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        ("stack/single-title-singlech", COMMIT_ONE),
    ])?;
    fixture.write_prs([pr_json(
        11,
        "OPEN",
        "stack/single-title-singlech",
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "single title",
        "single body",
    )])?;

    let output = fixture.run(["get", "11"])?;

    assert_success(&output);
    let jj = fixture.command_log("jj")?;
    assert!(
        jj.iter().any(|args| {
            args == &[
                "git".to_owned(),
                "fetch".to_owned(),
                "--remote".to_owned(),
                "origin".to_owned(),
                "--branch".to_owned(),
                "stack/single-title-singlech".to_owned(),
            ]
        }),
        "get should fetch only the target PR branch: {jj:#?}"
    );
    assert!(
        jj.iter().any(|args| args
            == &[
                "bookmark".to_owned(),
                "set".to_owned(),
                "jj-stack/frozen/pr-11".to_owned(),
                "-r".to_owned(),
                COMMIT_ONE.to_owned(),
            ]),
        "get should freeze the target PR head: {jj:#?}"
    );
    assert!(
        !jj.iter()
            .any(|args| args.contains(&"jj-stack/frozen/pr-12".to_owned())),
        "single-PR import should not infer descendants: {jj:#?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("next: `jj new jj-stack/frozen/pr-11`"),
        "get should print the next command: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(fixture.cache_entry("singlechange")?["pr_number"], json!(11));
    fixture.cleanup()
}

#[test]
fn integration_sync_rebases_then_submits() -> anyhow::Result<()> {
    let fixture = Fixture::new("sync-rebase-submit")?;
    fixture.write_jj_logs(&[
        stack_log(&[change_record(
            "changeone",
            COMMIT_ONE,
            &[ROOT_PARENT],
            "change title",
            "change title\n\nchange body",
        )]),
        stack_log(&[change_record(
            "changeone",
            COMMIT_ONE,
            &[REMOTE_TRUNK],
            "change title",
            "change title\n\nchange body",
        )]),
    ])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", REMOTE_TRUNK)],
        [(COMMIT_ONE, "main", REMOTE_TRUNK)],
        [(ROOT_PARENT, REMOTE_TRUNK, true)],
    )?;
    fixture.write_branch_heads([("main", REMOTE_TRUNK)])?;
    fixture.write_pr_numbers([("stack/change-title-changeon", 9)])?;
    fixture.write_prs([])?;

    // Submit is opt-in, so pass --submit to exercise the rebase-then-submit path.
    let output = fixture.run(["sync", "--submit"])?;

    assert_success(&output);
    let jj = fixture.command_log("jj")?;
    let rebase = index_of(&jj, &["rebase", "-s", COMMIT_ONE, "-d", "main"]);
    let gh = fixture.gh_requests()?;
    let create = gh.iter().position(|request| {
        request_args(request).starts_with(&[
            "api".to_owned(),
            "-X".to_owned(),
            "POST".to_owned(),
            "repos/owner/repo/pulls".to_owned(),
        ])
    });
    assert!(rebase.is_some(), "sync should run jj rebase: {jj:#?}");
    assert!(create.is_some(), "sync should submit after rebase: {gh:#?}");
    fixture.cleanup()
}

#[test]
fn integration_sync_cleans_up_merged_branch_and_preserves_unmerged() -> anyhow::Result<()> {
    let fixture = Fixture::new("sync-cleanup")?;
    let merged = "stack/merged-feature-mergedch";
    let orphan = "stack/orphan-feature-orphanch";
    let merged_commit = COMMIT_ONE;
    let orphan_commit = COMMIT_TWO;
    // Empty owned stack: everything has already merged.
    fixture.write_jj_logs(&[String::new()])?;
    fixture.write_git_maps(
        [("main", REMOTE_TRUNK), ("origin/main", REMOTE_TRUNK)],
        [],
        [
            // The merged branch's commit is in trunk; the orphan's is not.
            (merged_commit, REMOTE_TRUNK, true),
            (orphan_commit, REMOTE_TRUNK, false),
        ],
    )?;
    fixture.write_branch_heads([
        ("main", REMOTE_TRUNK),
        (merged, merged_commit),
        (orphan, orphan_commit),
    ])?;
    fixture.write_local_bookmarks([
        ("main", REMOTE_TRUNK),
        (merged, merged_commit),
        (orphan, orphan_commit),
    ])?;
    fixture.write_cleanup_stack_bookmarks([merged, orphan])?;
    fixture.write_prs([merge_pr_json(
        91,
        "MERGED",
        merged,
        "main",
        merged_commit,
        REMOTE_TRUNK,
    )])?;

    let output = fixture.run(["sync"])?;

    assert_success(&output);
    let jj = fixture.command_log("jj")?;
    // The merged branch is deleted locally and the deletion is pushed.
    assert!(
        index_of(&jj, &["bookmark", "delete", merged]).is_some(),
        "sync should delete the merged branch locally: {jj:#?}"
    );
    assert!(
        jj.iter().any(|command| {
            command.starts_with(
                &["git", "push", "--remote", "origin", "--bookmark", merged].map(str::to_owned),
            )
        }),
        "sync should push the deletion of the merged branch: {jj:#?}"
    );
    // The unmerged orphan branch is preserved.
    assert!(
        index_of(&jj, &["bookmark", "delete", orphan]).is_none(),
        "sync must not delete the unmerged branch: {jj:#?}"
    );
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        report.contains("1 merged branch(es) cleaned"),
        "sync should report one cleaned branch:\n{report}"
    );
    fixture.cleanup()
}

#[test]
fn integration_noop_submit_skips_push_and_pr_mutation() -> anyhow::Result<()> {
    let fixture = Fixture::new("noop-submit")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [(COMMIT_ONE, "main", ROOT_PARENT)],
        [],
    )?;
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        ("stack/change-title-changeon", COMMIT_ONE),
    ])?;
    fixture.write_local_bookmarks([("stack/change-title-changeon", COMMIT_ONE)])?;
    fixture.write_local_bookmarks([("stack/change-title-changeon", COMMIT_ONE)])?;
    fixture.write_prs([pr_json(
        9,
        "OPEN",
        "stack/change-title-changeon",
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "change title",
        "change body",
    )])?;
    fixture.write_cache(
        "changeone",
        9,
        "stack/change-title-changeon",
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "change title",
        "change body",
    )?;
    fixture.write_comments([(
        "9",
        vec![json!({
            "id": 101,
            "body": stack_comment_body(&[("changeone", 9, "stack/change-title-changeon", "main", "change title")], "changeone"),
            "userLogin": "octocat",
            "updatedAt": "2026-06-03T17:00:00Z",
        })],
    )])?;

    let output = fixture.run(["submit"])?;

    assert_success(&output);
    assert!(
        !fixture
            .command_log("git")?
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "push")),
        "no-op submit should not push"
    );
    let gh = fixture.gh_requests()?;
    assert!(!contains_gh(
        &gh,
        &["api", "-X", "POST", "repos/owner/repo/pulls"]
    ));
    assert!(!contains_gh(
        &gh,
        &["api", "-X", "PATCH", "repos/owner/repo/pulls/9"]
    ));
    fixture.cleanup()
}

#[test]
fn integration_submit_refuses_open_branch_pr_without_cache() -> anyhow::Result<()> {
    let fixture = Fixture::new("branch-without-cache")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [(COMMIT_ONE, "main", ROOT_PARENT)],
        [],
    )?;
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        ("stack/change-title-changeon", COMMIT_ONE),
    ])?;
    fixture.write_prs([pr_json(
        9,
        "OPEN",
        "stack/change-title-changeon",
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "change title",
        "change body",
    )])?;

    let output = fixture.run(["submit"])?;

    assert!(
        !output.status.success(),
        "branch without a local tracked bookmark should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr
            .contains("local head bookmark `stack/change-title-changeon` is missing or conflicted"),
        "{stderr}"
    );
    let gh = fixture.gh_requests()?;
    assert!(!contains_gh(
        &gh,
        &["api", "-X", "POST", "repos/owner/repo/pulls"]
    ));
    assert!(
        !fixture.cache_path().exists(),
        "failed submit should not recover or write cache"
    );
    fixture.cleanup()
}

#[test]
fn integration_submit_updates_existing_pr_from_tracked_bookmark_without_cache() -> anyhow::Result<()>
{
    let fixture = Fixture::new("submit-no-cache-tracked-bookmark")?;
    let branch = "stack/change-title-changeon";
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_TWO,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nedited body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [(COMMIT_TWO, "main", ROOT_PARENT)],
        [],
    )?;
    fixture.write_branch_heads([("main", ROOT_PARENT), (branch, COMMIT_ONE)])?;
    fixture.write_local_bookmarks([(branch, COMMIT_TWO)])?;
    fixture.write_prs([pr_json(
        9,
        "OPEN",
        branch,
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "change title",
        "old body",
    )])?;

    let output = fixture.run(["submit"])?;

    assert_success(&output);
    let prs =
        serde_json::from_str::<Vec<Value>>(&fs::read_to_string(fixture.root.join("prs.json"))?)?;
    assert_eq!(
        prs.len(),
        1,
        "submit should update, not duplicate: {prs:#?}"
    );
    assert_eq!(prs[0]["number"], json!(9));
    assert_eq!(prs[0]["headRefName"], json!(branch));
    assert_eq!(prs[0]["headRefOid"], json!(COMMIT_TWO));
    assert_eq!(prs[0]["body"], json!("edited body"));
    assert!(
        fixture.cache_path().exists(),
        "submit should rebuild local cache/cache hints after live discovery"
    );
    let gh = fixture.gh_requests()?;
    assert!(contains_gh(
        &gh,
        &["api", "-X", "PATCH", "repos/owner/repo/pulls/9"]
    ));
    assert!(!contains_gh(
        &gh,
        &["api", "-X", "POST", "repos/owner/repo/pulls"]
    ));
    fixture.cleanup()
}

#[test]
fn integration_submit_ignores_stale_cache_when_tracked_bookmark_matches_live_pr()
-> anyhow::Result<()> {
    let fixture = Fixture::new("submit-stale-cache-live-bookmark")?;
    let stale_branch = "stack/stale-cache-branch";
    let live_branch = "stack/live-branch-changeon";
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_TWO,
        &[ROOT_PARENT],
        "live title",
        "live title\n\nnew body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [(COMMIT_TWO, "main", ROOT_PARENT)],
        [],
    )?;
    fixture.write_branch_heads([("main", ROOT_PARENT), (live_branch, COMMIT_ONE)])?;
    fixture.write_local_bookmarks([(live_branch, COMMIT_TWO)])?;
    fixture.write_prs([pr_json(
        9,
        "OPEN",
        live_branch,
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "old title",
        "old body",
    )])?;
    fixture.write_cache(
        "changeone",
        8,
        stale_branch,
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "stale title",
        "stale body",
    )?;

    let output = fixture.run(["submit"])?;

    assert_success(&output);
    let prs =
        serde_json::from_str::<Vec<Value>>(&fs::read_to_string(fixture.root.join("prs.json"))?)?;
    assert_eq!(
        prs.len(),
        1,
        "submit should update, not duplicate: {prs:#?}"
    );
    assert_eq!(prs[0]["number"], json!(9));
    assert_eq!(prs[0]["headRefName"], json!(live_branch));
    assert_eq!(prs[0]["headRefOid"], json!(COMMIT_TWO));
    assert_eq!(prs[0]["title"], json!("live title"));

    let entry = fixture.cache_entry("changeone")?;
    assert_eq!(entry["pr_number"], json!(9));
    assert_eq!(entry["head_branch"], json!(live_branch));

    let gh = fixture.gh_requests()?;
    assert!(contains_gh(
        &gh,
        &["api", "-X", "PATCH", "repos/owner/repo/pulls/9"]
    ));
    assert!(!contains_gh(
        &gh,
        &["api", "-X", "POST", "repos/owner/repo/pulls"]
    ));
    fixture.cleanup()
}

#[test]
fn integration_merge_dry_run_checks_without_mutating() -> anyhow::Result<()> {
    let fixture = Fixture::new("merge-dry-run")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [],
        [],
    )?;
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        ("stack/change-title-changeon", COMMIT_ONE),
    ])?;
    fixture.write_local_bookmarks([("stack/change-title-changeon", COMMIT_ONE)])?;
    fixture.write_prs([merge_pr_json(
        9,
        "OPEN",
        "stack/change-title-changeon",
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
    )])?;
    fixture.write_cache(
        "changeone",
        9,
        "stack/change-title-changeon",
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "change title",
        "change body",
    )?;

    let output = fixture.run(["merge", "--dry-run"])?;

    assert_success(&output);
    assert!(!contains_gh(&fixture.gh_requests()?, &["pr", "merge", "9"]));
    assert!(
        !fixture
            .command_log("jj")?
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "abandon")),
        "dry-run merge should not abandon local changes"
    );
    fixture.cleanup()
}

#[test]
fn integration_merge_dry_run_discovers_pr_without_cache() -> anyhow::Result<()> {
    let fixture = Fixture::new("merge-without-cache")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [],
        [],
    )?;
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        ("stack/change-title-changeon", COMMIT_ONE),
    ])?;
    fixture.write_local_bookmarks([("stack/change-title-changeon", COMMIT_ONE)])?;
    fixture.write_prs([merge_pr_json(
        9,
        "OPEN",
        "stack/change-title-changeon",
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
    )])?;

    let output = fixture.run(["merge", "--dry-run"])?;

    assert_success(&output);
    assert!(contains_gh(&fixture.gh_requests()?, &["pr", "view", "9"]));
    assert!(!contains_gh(&fixture.gh_requests()?, &["pr", "merge", "9"]));
    fixture.cleanup()
}

#[test]
fn integration_merge_dry_run_ignores_stale_cache_for_live_bookmark_pr() -> anyhow::Result<()> {
    let fixture = Fixture::new("merge-stale-cache-live-bookmark")?;
    let live_branch = "stack/change-title-changeon";
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [],
        [],
    )?;
    fixture.write_branch_heads([("main", ROOT_PARENT), (live_branch, COMMIT_ONE)])?;
    fixture.write_local_bookmarks([(live_branch, COMMIT_ONE)])?;
    fixture.write_prs([merge_pr_json(
        9,
        "OPEN",
        live_branch,
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
    )])?;
    fixture.write_cache(
        "changeone",
        8,
        "stack/stale-branch-changeon",
        "main",
        COMMIT_TWO,
        ROOT_PARENT,
        "stale title",
        "stale body",
    )?;

    let output = fixture.run(["merge", "--dry-run"])?;

    assert_success(&output);
    let gh = fixture.gh_requests()?;
    assert!(contains_gh(&gh, &["pr", "view", "9"]));
    assert!(!contains_gh(&gh, &["pr", "merge", "9"]));
    fixture.cleanup()
}

#[test]
fn integration_clean_two_pr_merge_merges_bottom_then_top() -> anyhow::Result<()> {
    let fixture = Fixture::new("merge-two-clean")?;
    let bottom_branch = "stack/bottom-title-bottomch";
    let top_branch = "stack/top-title-topchang";
    // The fast-forward merge resolves the stack once; a single jj-log entry
    // means every `jj log` read returns the same two-change stack.
    fixture.write_jj_logs(&[stack_log(&[
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
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", REMOTE_TRUNK)],
        [],
        // Trunk freshness check, plus the fast-forward ancestry check
        // (remote trunk must be an ancestor of the new top commit).
        [
            (ROOT_PARENT, REMOTE_TRUNK, true),
            (REMOTE_TRUNK, COMMIT_TWO, true),
        ],
    )?;
    fixture.write_branch_heads([
        ("main", ROOT_PARENT),
        (bottom_branch, COMMIT_ONE),
        (top_branch, COMMIT_TWO),
    ])?;
    fixture.write_local_bookmarks([
        ("main", ROOT_PARENT),
        (bottom_branch, COMMIT_ONE),
        (top_branch, COMMIT_TWO),
    ])?;
    fixture.enable_auto_merge_on_trunk("main")?;
    fixture.write_prs([
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
    fixture.write_cache(
        "bottomchange",
        11,
        bottom_branch,
        "main",
        COMMIT_ONE,
        ROOT_PARENT,
        "bottom title",
        "bottom body",
    )?;
    fixture.write_cache(
        "topchange",
        12,
        top_branch,
        bottom_branch,
        COMMIT_TWO,
        COMMIT_ONE,
        "top title",
        "top body",
    )?;

    // The merge re-points the top PR onto trunk, then fast-forwards trunk over
    // the whole stack in a single push; GitHub auto-merges both PRs by
    // reachability. No squash, no branch deletes, no abandons.
    let output = fixture.run(["merge"])?;

    assert_success(&output);
    let gh = fixture.gh_requests()?;
    // The top PR is retargeted from the bottom branch onto trunk.
    let top_retarget = gh.iter().find(|request| {
        request_args(request).starts_with(&[
            "api".to_owned(),
            "-X".to_owned(),
            "PATCH".to_owned(),
            "repos/owner/repo/pulls/12".to_owned(),
        ])
    });
    let Some(top_retarget) = top_retarget else {
        panic!("expected top PR retarget onto trunk: {gh:#?}");
    };
    assert!(
        request_args(top_retarget).contains(&"base=main".to_owned()),
        "top PR should be retargeted to trunk: {top_retarget:#?}"
    );
    // The old squash path is gone: no `gh pr merge` calls.
    assert!(
        !gh.iter().any(|request| {
            let args = request_args(request);
            args.first().map(String::as_str) == Some("pr")
                && args.get(1).map(String::as_str) == Some("merge")
        }),
        "merge must not squash-merge via gh pr merge: {gh:#?}"
    );
    let jj = fixture.command_log("jj")?;
    // Trunk is fast-forwarded over the top of the stack and pushed exactly once.
    assert!(
        index_of(&jj, &["bookmark", "set", "main", "-r", COMMIT_TWO]).is_some(),
        "merge should fast-forward trunk to the top commit: {jj:#?}"
    );
    let pushes = jj
        .iter()
        .filter(|command| {
            command.starts_with(&[
                "git".to_owned(),
                "push".to_owned(),
                "--remote".to_owned(),
                "origin".to_owned(),
                "--bookmark".to_owned(),
                "main".to_owned(),
            ])
        })
        .count();
    assert_eq!(pushes, 1, "merge should push trunk exactly once: {jj:#?}");
    // Nothing is abandoned in the fast-forward model.
    assert!(
        !jj.iter()
            .any(|command| command.first().map(String::as_str) == Some("abandon")),
        "merge must not abandon changes: {jj:#?}"
    );
    // Both merged stack branches are cleaned up: each local bookmark deleted...
    for branch in [bottom_branch, top_branch] {
        assert!(
            index_of(&jj, &["bookmark", "delete", branch]).is_some(),
            "merge should delete merged branch `{branch}` locally: {jj:#?}"
        );
    }
    // ...and both deletions pushed in a SINGLE batched `jj git push` (not one
    // push per branch). The trunk push is separate, so exactly two pushes total.
    let branch_deletion_push = jj.iter().find(|command| {
        command.starts_with(&["git".to_owned(), "push".to_owned()])
            && command.contains(&bottom_branch.to_owned())
    });
    let Some(branch_deletion_push) = branch_deletion_push else {
        panic!("expected a batched branch-deletion push: {jj:#?}");
    };
    assert!(
        branch_deletion_push.contains(&top_branch.to_owned()),
        "both merged branches must be deleted in one push: {branch_deletion_push:#?}"
    );
    let total_pushes = jj
        .iter()
        .filter(|command| command.starts_with(&["git".to_owned(), "push".to_owned()]))
        .count();
    assert_eq!(
        total_pushes, 2,
        "exactly two pushes: the trunk fast-forward and one batched branch deletion: {jj:#?}"
    );
    let prs: Vec<Value> =
        serde_json::from_str(&fs::read_to_string(fixture.root.join("prs.json"))?)?;
    assert!(
        prs.iter().all(|pr| pr["state"] == "MERGED"),
        "all PRs should be merged by reachability: {prs:#?}"
    );
    fixture.cleanup()
}

#[test]
fn integration_sync_divergence_stops_before_rebase() -> anyhow::Result<()> {
    let fixture = Fixture::new("sync-divergence")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", REMOTE_TRUNK)],
        [],
        [(ROOT_PARENT, REMOTE_TRUNK, false)],
    )?;

    let output = fixture.run(["sync"])?;

    assert!(!output.status.success(), "divergent sync should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(ROOT_PARENT), "{stderr}");
    assert!(stderr.contains(REMOTE_TRUNK), "{stderr}");
    assert!(
        index_of(
            &fixture.command_log("jj")?,
            &["rebase", "-s", COMMIT_ONE, "-d", "main"]
        )
        .is_none(),
        "divergence should stop before rebase"
    );
    fixture.cleanup()
}

#[test]
fn integration_non_default_workspace_writes_cache_to_backing_repo() -> anyhow::Result<()> {
    let fixture = Fixture::new_non_default("non-default-workspace")?;
    fixture.write_jj_logs(&[stack_log(&[change_record(
        "changeone",
        COMMIT_ONE,
        &[ROOT_PARENT],
        "change title",
        "change title\n\nchange body",
    )])])?;
    fixture.write_git_maps(
        [("main", ROOT_PARENT), ("origin/main", ROOT_PARENT)],
        [(COMMIT_ONE, "main", ROOT_PARENT)],
        [],
    )?;
    fixture.write_branch_heads([("main", ROOT_PARENT)])?;
    fixture.write_pr_numbers([("stack/change-title-changeon", 9)])?;
    fixture.write_prs([])?;

    let output = fixture.run(["submit"])?;

    assert_success(&output);
    assert!(
        fixture.cache_path().exists(),
        "backing repo cache should exist"
    );
    assert!(
        !fixture
            .workspace
            .join(".jj")
            .join(CONFIG_PREFIX)
            .join("cache.sqlite")
            .exists(),
        "non-default workspace should not receive workflow cache"
    );
    fixture.cleanup()
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    repo_dir: PathBuf,
    bin_dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> anyhow::Result<Self> {
        let root = unique_dir(name)?;
        let workspace = root.join("workspace");
        let repo_dir = workspace.join(".jj").join("repo");
        fs::create_dir_all(&repo_dir)?;
        fs::create_dir_all(workspace.join(".jj"))?;
        let fixture = Self {
            bin_dir: root.join("bin"),
            root,
            workspace,
            repo_dir,
        };
        fixture.install_fakes()?;
        Ok(fixture)
    }

    fn new_non_default(name: &str) -> anyhow::Result<Self> {
        let root = unique_dir(name)?;
        let workspace = root.join("workspace");
        let repo_dir = root.join("backing").join(".jj").join("repo");
        fs::create_dir_all(&repo_dir)?;
        fs::create_dir_all(workspace.join(".jj"))?;
        fs::write(
            workspace.join(".jj").join("repo"),
            "../../backing/.jj/repo\n",
        )?;
        let fixture = Self {
            bin_dir: root.join("bin"),
            root,
            workspace,
            repo_dir,
        };
        fixture.install_fakes()?;
        Ok(fixture)
    }

    fn install_fakes(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.bin_dir)?;
        write_executable(&self.bin_dir.join("jj"), JJ_FAKE)?;
        write_executable(&self.bin_dir.join("git"), GIT_FAKE)?;
        write_executable(&self.bin_dir.join("gh"), GH_FAKE)
    }

    fn run<const N: usize>(&self, args: [&str; N]) -> anyhow::Result<Output> {
        let old_path = env::var("PATH").unwrap_or_default();
        Command::new(env!("CARGO_BIN_EXE_jj-stack"))
            .args(args)
            .current_dir(&self.workspace)
            .env("PATH", format!("{}:{old_path}", self.bin_dir.display()))
            .env("STACK_FAKE_ROOT", &self.root)
            .env("STACK_FAKE_WORKSPACE", &self.workspace)
            .output()
            .map_err(Into::into)
    }

    fn write_jj_logs(&self, logs: &[String]) -> anyhow::Result<()> {
        for (index, log) in logs.iter().enumerate() {
            fs::write(self.root.join(format!("jj-log-{}", index + 1)), log)?;
        }
        Ok(())
    }

    fn write_git_maps<const R: usize, const M: usize, const A: usize>(
        &self,
        rev_parse: [(&str, &str); R],
        merge_bases: [(&str, &str, &str); M],
        ancestors: [(&str, &str, bool); A],
    ) -> anyhow::Result<()> {
        fs::write(
            self.root.join("rev-parse.json"),
            serde_json::to_string(&rev_parse.into_iter().collect::<BTreeMap<_, _>>())?,
        )?;
        let merge_bases = merge_bases
            .into_iter()
            .map(|(left, right, output)| (format!("{left}\n{right}"), output))
            .collect::<BTreeMap<_, _>>();
        fs::write(
            self.root.join("merge-bases.json"),
            serde_json::to_string(&merge_bases)?,
        )?;
        let ancestors = ancestors
            .into_iter()
            .map(|(left, right, success)| (format!("{left}\n{right}"), success))
            .collect::<BTreeMap<_, _>>();
        fs::write(
            self.root.join("ancestors.json"),
            serde_json::to_string(&ancestors)?,
        )
        .map_err(Into::into)
    }

    fn write_branch_heads<const N: usize>(&self, heads: [(&str, &str); N]) -> anyhow::Result<()> {
        fs::write(
            self.root.join("branch-heads.json"),
            serde_json::to_string(&heads.into_iter().collect::<BTreeMap<_, _>>())?,
        )
        .map_err(Into::into)
    }

    fn write_local_bookmarks<const N: usize>(
        &self,
        heads: [(&str, &str); N],
    ) -> anyhow::Result<()> {
        fs::write(
            self.root.join("local-bookmarks.json"),
            serde_json::to_string(&heads.into_iter().collect::<BTreeMap<_, _>>())?,
        )
        .map_err(Into::into)
    }

    fn write_pr_numbers<const N: usize>(&self, numbers: [(&str, u64); N]) -> anyhow::Result<()> {
        fs::write(
            self.root.join("pr-numbers.json"),
            serde_json::to_string(&numbers.into_iter().collect::<BTreeMap<_, _>>())?,
        )
        .map_err(Into::into)
    }

    fn write_prs<const N: usize>(&self, prs: [Value; N]) -> anyhow::Result<()> {
        fs::write(
            self.root.join("prs.json"),
            serde_json::to_string(&prs.into_iter().collect::<Vec<_>>())?,
        )
        .map_err(Into::into)
    }

    /// Enable the mock's GitHub reachability auto-merge: a fast-forward push of
    /// `trunk` marks every open PR retargeted onto it as MERGED.
    fn enable_auto_merge_on_trunk(&self, trunk: &str) -> anyhow::Result<()> {
        fs::write(
            self.root.join("auto-merge-trunk.json"),
            serde_json::to_string(trunk)?,
        )
        .map_err(Into::into)
    }

    /// Make the merged-branch cleanup phase see these local stack bookmarks.
    fn write_cleanup_stack_bookmarks<const N: usize>(
        &self,
        bookmarks: [&str; N],
    ) -> anyhow::Result<()> {
        fs::write(
            self.root.join("cleanup-stack-bookmarks.json"),
            serde_json::to_string(&bookmarks.to_vec())?,
        )
        .map_err(Into::into)
    }

    fn write_comments<const N: usize>(
        &self,
        comments: [(&str, Vec<Value>); N],
    ) -> anyhow::Result<()> {
        fs::write(
            self.root.join("comments.json"),
            serde_json::to_string(&comments.into_iter().collect::<BTreeMap<_, _>>())?,
        )
        .map_err(Into::into)
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
    ) -> anyhow::Result<()> {
        let path = self.cache_path();
        let Some(parent) = path.parent() else {
            anyhow::bail!("cache path has no parent: {}", path.display());
        };
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

    fn cache_entry(&self, change_id: &str) -> anyhow::Result<Value> {
        let conn = Connection::open(self.cache_path())?;
        let mut statement = conn.prepare(
            "SELECT pr_number, head_branch, base_branch, head_sha, base_sha, title, body
               FROM pr_cache
              WHERE repo = 'owner/repo' AND change_id = ?1",
        )?;
        let entry = statement.query_row([change_id], |row| {
            Ok(json!({
                "pr_number": row.get::<_, i64>(0)?,
                "head_branch": row.get::<_, String>(1)?,
                "base_branch": row.get::<_, String>(2)?,
                "head_sha": row.get::<_, String>(3)?,
                "base_sha": row.get::<_, String>(4)?,
                "title": row.get::<_, String>(5)?,
                "body": row.get::<_, String>(6)?,
            }))
        })?;
        Ok(entry)
    }

    fn gh_requests(&self) -> anyhow::Result<Vec<Value>> {
        read_jsonl(&self.root.join("gh-requests.jsonl"))
    }

    fn command_log(&self, name: &str) -> anyhow::Result<Vec<Vec<String>>> {
        Ok(
            read_jsonl(&self.root.join(format!("{name}-requests.jsonl")))?
                .into_iter()
                .filter_map(|value| {
                    value.get("args").and_then(Value::as_array).map(|args| {
                        args.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                })
                .collect(),
        )
    }

    fn cache_path(&self) -> PathBuf {
        self.repo_dir.join(CONFIG_PREFIX).join("cache.sqlite")
    }

    fn cleanup(self) -> anyhow::Result<()> {
        fs::remove_dir_all(self.root).map_err(Into::into)
    }
}

fn init_cache_schema(conn: &Connection) -> anyhow::Result<()> {
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn contains_gh(requests: &[Value], prefix: &[&str]) -> bool {
    requests.iter().any(|request| {
        let args = request_args(request);
        args.iter()
            .map(String::as_str)
            .take(prefix.len())
            .eq(prefix.iter().copied())
    })
}

fn contains_gh_field(requests: &[Value], field: &str) -> bool {
    requests
        .iter()
        .any(|request| request_args(request).contains(&field.to_owned()))
}

fn request_args(request: &Value) -> Vec<String> {
    request
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn index_of(args: &[Vec<String>], expected: &[&str]) -> Option<usize> {
    let expected = expected
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>();
    args.iter().position(|args| args == &expected)
}

fn read_jsonl(path: &Path) -> anyhow::Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn write_executable(path: &Path, contents: &str) -> anyhow::Result<()> {
    fs::write(path, contents)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(Into::into)
}

fn unique_dir(name: &str) -> anyhow::Result<PathBuf> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = env::temp_dir().join(format!("stack-it-{name}-{}-{nanos}", std::process::id()));
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
        serde_json::to_string(change_id).unwrap_or_else(|error| panic!("{error}")),
        serde_json::to_string(commit_id).unwrap_or_else(|error| panic!("{error}")),
        serde_json::to_string(parent_ids).unwrap_or_else(|error| panic!("{error}")),
        serde_json::to_string(title).unwrap_or_else(|error| panic!("{error}")),
        serde_json::to_string(description).unwrap_or_else(|error| panic!("{error}")),
        "false".to_owned(),
        "false".to_owned(),
    ]
    .join(&STACK_FIELD_SEPARATOR.to_string())
        + &STACK_RECORD_SEPARATOR.to_string()
}

fn pr_json(
    number: u64,
    state: &str,
    head: &str,
    base: &str,
    head_sha: &str,
    base_sha: &str,
    title: &str,
    body: &str,
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
        "author": {
            "login": "octocat",
        },
        "title": title,
        "body": body,
        "createdAt": "2026-06-03T12:34:56Z"
    })
}

fn merge_pr_json(
    number: u64,
    state: &str,
    head: &str,
    base: &str,
    head_sha: &str,
    base_sha: &str,
) -> Value {
    let mut value = pr_json(
        number,
        state,
        head,
        base,
        head_sha,
        base_sha,
        "change title",
        "change body",
    );
    let object = value
        .as_object_mut()
        .unwrap_or_else(|| panic!("PR JSON should be object"));
    object.insert("isDraft".to_owned(), json!(false));
    object.insert("reviewDecision".to_owned(), json!("APPROVED"));
    object.insert("mergeable".to_owned(), json!("MERGEABLE"));
    object.insert("mergeStateStatus".to_owned(), json!("CLEAN"));
    object.insert(
        "statusCheckRollup".to_owned(),
        json!([{ "context": "ci", "state": "SUCCESS" }]),
    );
    object.insert("autoMergeRequest".to_owned(), Value::Null);
    value
}

fn stack_comment_body(rows: &[(&str, u64, &str, &str, &str)], current_change_id: &str) -> String {
    let mut body = "<!-- stack:v1 -->\nStack for owner/repo\n\n".to_owned();
    for (change_id, number, _, _, title) in rows.iter().rev() {
        let label = format!("[{title} #{number}](https://github.com/owner/repo/pull/{number})");
        let is_current = *change_id == current_change_id;
        let label = if is_current {
            format!("**{label}**")
        } else {
            label
        };
        let current_marker = if is_current { " 👈" } else { "" };
        let short_change_id = change_id.chars().take(8).collect::<String>();
        body.push_str(&format!(
            "- {label} _{short_change_id}_ · 2026-06-03 12:34:56{current_marker}\n"
        ));
    }
    body.push_str("- main\n");
    body.push_str("\n");
    if let Some((_, number, _, _, _)) = rows
        .iter()
        .find(|(change_id, _, _, _, _)| *change_id == current_change_id)
    {
        body.push_str(&format!(
            "Check out this stack: `jj-stack get https://github.com/owner/repo/pull/{number}`\n"
        ));
    }
    body.push_str("Pull/update this stack: `jj-stack sync`\n");
    body.push_str("Publish local edits: `jj-stack submit`\n");
    body.push_str("Merge when ready: `jj-stack merge`\n");
    body
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
    # Remote trunk bookmark resolution: `log --no-graph -r <trunk>@<remote> -T commit_id`.
    # Resolve from the same rev-parse map git uses so the two views agree.
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
            value = refs.get("origin/main") or refs.get("main") or heads.get("main") or "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            print(value)
            sys.exit(0)
        if "@" in rev:
            trunk, remote = rev.split("@", 1)
            refs_path = root / "rev-parse.json"
            refs = json.loads(refs_path.read_text()) if refs_path.exists() else {}
            value = refs.get(remote + "/" + trunk)
            if value is None:
                print("missing remote bookmark " + rev, file=sys.stderr)
                sys.exit(1)
            print(value)
            sys.exit(0)
        heads_path = root / "branch-heads.json"
        heads = json.loads(heads_path.read_text()) if heads_path.exists() else {}
        local_heads = load("local-bookmarks.json", {})
        value = local_heads.get(rev)
        if value is not None:
            print(value)
            sys.exit(0)
        print("missing local bookmark " + rev, file=sys.stderr)
        sys.exit(1)
    if "-T" in args and "-r" in args:
        rev = args[args.index("-r") + 1]
        log_path = root / "jj-log-1"
        if len(rev) == 40 and log_path.exists():
            for record in log_path.read_text().split("\x1e"):
                if not record:
                    continue
                fields = record.split("\x1f")
                if len(fields) >= 2 and json.loads(fields[1]) == rev:
                    sys.stdout.write(record + "\x1e")
                    sys.exit(0)
            sys.exit(0)
    counter_path = root / "jj-log-counter"
    count = int(counter_path.read_text()) if counter_path.exists() else 0
    count += 1
    counter_path.write_text(str(count))
    path = root / f"jj-log-{count}"
    if not path.exists():
        path = root / "jj-log-1"
    if path.exists():
        sys.stdout.write(path.read_text())
    sys.exit(0)
if args[:3] == ["bookmark", "set", "--allow-backwards"] and "-r" in args:
    branch = args[3]
    commit = args[args.index("-r") + 1]
    heads = load("local-bookmarks.json", {})
    heads[branch] = commit
    save("local-bookmarks.json", heads)
    sys.exit(0)
if args[:2] == ["bookmark", "list"] and "--revision" in args:
    rev = args[args.index("--revision") + 1]
    local_heads = load("local-bookmarks.json", {})
    for branch, commit in sorted(local_heads.items()):
        if commit == rev:
            print(f"{branch}\t")
    sys.exit(0)
if args[:3] == ["bookmark", "list", "jj-stack/frozen/*"] and "-T" in args:
    sys.exit(0)
if args[:2] == ["bookmark", "list"] and len(args) > 2 and args[2] == "-T":
    # Merged-branch cleanup lists every local bookmark (name + empty remote).
    # Opt-in per-fixture so unrelated sync tests see no stack bookmarks here.
    for branch in load("cleanup-stack-bookmarks.json", []):
        print(f"{branch}\t")
    sys.exit(0)
if args[:2] == ["bookmark", "delete"]:
    heads = load("local-bookmarks.json", {})
    heads.pop(args[2], None)
    save("local-bookmarks.json", heads)
    sys.exit(0)
if args[:2] == ["bookmark", "set"] and "-r" in args:
    branch = args[2]
    commit = args[args.index("-r") + 1]
    heads = load("local-bookmarks.json", {})
    heads[branch] = commit
    save("local-bookmarks.json", heads)
    sys.exit(0)
if args[:2] == ["git", "push"] and "--bookmark" in args:
    branch = args[args.index("--bookmark") + 1]
    local_heads = load("local-bookmarks.json", {})
    if branch in local_heads:
        heads = load("branch-heads.json", {})
        heads[branch] = local_heads[branch]
        save("branch-heads.json", heads)
    # Simulate GitHub's reachability-based auto-merge: when the configured trunk
    # bookmark is fast-forward pushed, every open PR retargeted onto it has its
    # head land in its base branch, so GitHub marks it merged. Opt-in per-fixture
    # so unrelated pushes (e.g. stack branches during submit) are unaffected.
    if load("auto-merge-trunk.json", None) == branch:
        prs = load("prs.json", [])
        changed = False
        for pr in prs:
            if pr["state"].upper() == "OPEN" and pr["baseRefName"] == branch:
                pr["state"] = "MERGED"
                changed = True
        if changed:
            save("prs.json", prs)
    sys.exit(0)
if args and args[0] == "new":
    sys.exit(0)
if args and args[0] in {"git", "bookmark", "rebase", "abandon", "edit"}:
    if args[:3] == ["bookmark", "list", "--all-remotes"] and "-T" in args:
        branch = args[3]
        heads_path = root / "branch-heads.json"
        heads = json.loads(heads_path.read_text()) if heads_path.exists() else {}
        if branch in heads:
            print("origin\ttracked\tok")
        sys.exit(0)
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
if args and args[0] == "merge-base":
    merge_bases = load("merge-bases.json", {})
    value = merge_bases.get(args[1] + "\n" + args[2])
    if value is None:
        print("missing merge-base", file=sys.stderr)
        sys.exit(1)
    print(value)
    sys.exit(0)
if args[:2] == ["ls-remote", "--heads"]:
    heads = load("branch-heads.json", {})
    value = heads.get(args[3])
    if value:
        print(f"{value}\trefs/heads/{args[3]}")
    sys.exit(0)
if args and args[0] == "rev-parse":
    refs = load("rev-parse.json", {})
    value = refs.get(args[1])
    if value is None:
        print("missing rev-parse", file=sys.stderr)
        sys.exit(1)
    print(value)
    sys.exit(0)
if args and args[0] == "push":
    sys.exit(0)

print("unconfigured git command: " + " ".join(args), file=sys.stderr)
sys.exit(1)
"#;

const GH_FAKE: &str = r#"#!/usr/bin/env python3
import json
import os
import sys
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
        "author": {
            "login": "octocat",
        },
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
    for pr in load("prs.json", []):
        if int(pr["number"]) == number:
            # verify_prs_merged polls `--json state --jq .state`; honor the jq so
            # it sees a bare state value rather than the full JSON object.
            if jq == ".state":
                print(pr["state"])
            else:
                print(json.dumps(pr))
            sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)
if args[:2] == ["pr", "merge"]:
    number = int(args[2])
    prs = load("prs.json", [])
    for pr in prs:
        if int(pr["number"]) == number:
            pr["state"] = "MERGED"
    save("prs.json", prs)
    sys.exit(0)
if args[:3] == ["api", "repos/owner/repo", "--jq"]:
    print("true")
    sys.exit(0)
if args[:1] == ["api"] and len(args) >= 2 and args[1].startswith("repos/owner/repo/pulls/") and "-X" not in args:
    number = int(args[1].rsplit("/", 1)[1])
    for pr in load("prs.json", []):
        if int(pr["number"]) == number:
            print(json.dumps(pr))
            sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)
if args[:2] == ["api", "--paginate"] and "/issues/" in args[2] and args[2].endswith("/comments"):
    pr_number = args[2].split("/issues/")[1].split("/")[0]
    for comment in load("comments.json", {}).get(pr_number, []):
        print(json.dumps(comment))
    sys.exit(0)
if args[:3] == ["api", "-X", "POST"] and args[3] == "repos/owner/repo/pulls":
    values = field_values()
    numbers = load("pr-numbers.json", {})
    number = int(numbers.get(values["head"], 100 + len(load("prs.json", []))))
    prs = load("prs.json", [])
    pr = pr_response(number, "OPEN", values["head"], values["base"], values["title"], values.get("body", ""))
    prs.append(pr)
    save("prs.json", prs)
    print(json.dumps(pr))
    sys.exit(0)
if args[:3] == ["api", "-X", "PATCH"] and args[3].startswith("repos/owner/repo/pulls/"):
    number = int(args[3].rsplit("/", 1)[1])
    values = field_values()
    prs = load("prs.json", [])
    for pr in prs:
        if int(pr["number"]) == number:
            # A retarget only sends `base`; preserve the PR's existing
            # state/title/body rather than requiring them in the PATCH args.
            base = values.get("base", pr["baseRefName"])
            title = values.get("title", pr.get("title", ""))
            body = values.get("body", pr.get("body", ""))
            pr.update(pr_response(number, pr["state"], pr["headRefName"], base, title, body))
            print(json.dumps(pr))
            save("prs.json", prs)
            sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)
if args[:3] == ["api", "-X", "POST"] and "/issues/" in args[3] and args[3].endswith("/comments"):
    pr_number = args[3].split("/issues/")[1].split("/")[0]
    comments = load("comments.json", {})
    values = field_values()
    next_id = 100 + sum(len(items) for items in comments.values()) + 1
    comments.setdefault(pr_number, []).append({
        "id": next_id,
        "body": values.get("body", ""),
        "userLogin": "octocat",
        "updatedAt": "2026-06-03T18:00:00Z",
    })
    save("comments.json", comments)
    print(json.dumps({"id": next_id}))
    sys.exit(0)
if args[:3] == ["api", "-X", "PATCH"] and "/issues/comments/" in args[3]:
    comment_id = int(args[3].rsplit("/", 1)[1])
    values = field_values()
    comments = load("comments.json", {})
    for items in comments.values():
        for comment in items:
            if int(comment["id"]) == comment_id:
                comment["body"] = values.get("body", "")
                save("comments.json", comments)
                print(json.dumps({"id": comment_id}))
                sys.exit(0)
    print("not found", file=sys.stderr)
    sys.exit(1)

print("unconfigured gh command: " + " ".join(args), file=sys.stderr)
sys.exit(1)
"#;
