//! Where a community skill's `SKILL.md` lives when its catalog entry is not a
//! GitHub link.
//!
//! - **ClawHub** entries carry only a slug; ClawHub's file API serves the raw
//!   `SKILL.md` for it.
//! - **skills.sh** entries point at `skills.sh/<owner>/<repo>/<skill>`, a
//!   listing of a GitHub repo. Repos keep skills in different directories, so
//!   the file is located at install time: the conventional directories first,
//!   then one recursive tree listing of the repo.
//!
//! LobeHub entries are system-prompt agents with no `SKILL.md` at all and stay
//! uninstallable.

use std::time::Duration;

use serde_json::Value;

const CLAWHUB_SKILLS_API: &str = "https://clawhub.ai/api/v1/skills";
const GITHUB_RAW: &str = "https://raw.githubusercontent.com";
const GITHUB_REPOS_API: &str = "https://api.github.com/repos";
/// Directories a skills.sh repo conventionally keeps a skill under, probed in
/// this order before listing the whole repo.
const SKILLS_SH_BASE_DIRS: [&str; 4] = ["", "skills/", ".agents/skills/", ".claude/skills/"];
const PROBE_TIMEOUT_SECS: u64 = 15;

/// A catalog value is spliced into a URL path, so it must be one plain segment.
fn is_safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Raw `SKILL.md` URL for a ClawHub skill slug.
pub(super) fn clawhub_download_url(slug: &str) -> Option<String> {
    is_safe_segment(slug).then(|| format!("{CLAWHUB_SKILLS_API}/{slug}/file?path=SKILL.md"))
}

/// A skills.sh listing, `https://skills.sh/<owner>/<repo>/<skill>`.
#[derive(Debug)]
pub(super) struct SkillsShRef<'a> {
    pub(super) owner: &'a str,
    pub(super) repo: &'a str,
    pub(super) skill: &'a str,
}

impl<'a> SkillsShRef<'a> {
    pub(super) fn parse(source_url: &'a str) -> Option<Self> {
        let rest = source_url.strip_prefix("https://skills.sh/")?;
        let mut parts = rest.trim_end_matches('/').split('/');
        let (owner, repo, skill) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || ![owner, repo, skill].into_iter().all(is_safe_segment) {
            return None;
        }
        Some(Self { owner, repo, skill })
    }

    /// Raw URLs of the conventional skill locations, most common first.
    pub(super) fn candidate_urls(&self) -> Vec<String> {
        SKILLS_SH_BASE_DIRS
            .iter()
            .map(|base| self.raw_url(&format!("{base}{}/SKILL.md", self.skill)))
            .collect()
    }

    fn raw_url(&self, path: &str) -> String {
        format!("{GITHUB_RAW}/{}/{}/HEAD/{path}", self.owner, self.repo)
    }

    /// Locate this skill's `SKILL.md` in its GitHub repo.
    pub(super) async fn resolve(&self) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(PROBE_TIMEOUT_SECS))
            .user_agent("openhuman-core")
            .build()
            .map_err(|e| format!("failed to build http client: {e}"))?;

        // Probe every conventional location at once: probed one after another,
        // each slow miss could spend the whole timeout before the next starts.
        let candidates = self.candidate_urls();
        let responses =
            futures::future::join_all(candidates.iter().map(|url| client.head(url).send())).await;
        for (url, response) in candidates.into_iter().zip(responses) {
            match response {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(url = %url, "[skill_registry] skills.sh SKILL.md found");
                    return Ok(url);
                }
                Ok(resp) => tracing::debug!(
                    url = %url,
                    status = resp.status().as_u16(),
                    "[skill_registry] skills.sh candidate missing"
                ),
                Err(error) => tracing::debug!(
                    url = %url,
                    error = %error,
                    "[skill_registry] skills.sh candidate probe failed"
                ),
            }
        }

        // Not in a conventional directory: one recursive listing finds it anywhere.
        let repo = format!("github.com/{}/{}", self.owner, self.repo);
        let tree_url = format!(
            "{GITHUB_REPOS_API}/{}/{}/git/trees/HEAD?recursive=1",
            self.owner, self.repo
        );
        tracing::info!(repo = %repo, skill = %self.skill, "[skill_registry] listing repo tree for skills.sh skill");
        let resp = client
            .get(&tree_url)
            .send()
            .await
            .map_err(|e| format!("could not list {repo} to locate '{}': {e}", self.skill))?;
        if !resp.status().is_success() {
            return Err(format!(
                "could not list {repo} to locate '{}' (GitHub returned {})",
                self.skill,
                resp.status().as_u16()
            ));
        }
        let tree: Value = resp
            .json()
            .await
            .map_err(|e| format!("could not read the {repo} file listing: {e}"))?;
        let skill = self.skill;
        match find_skill_md_in_tree(&tree, skill) {
            Ok(path) => Ok(self.raw_url(&path)),
            Err(TreeMiss::Absent) => Err(format!(
                "'{skill}' is listed on skills.sh, but {repo} has no {skill}/SKILL.md"
            )),
            Err(TreeMiss::Truncated) => Err(format!(
                "{repo} is too large for GitHub to list in one response, so '{skill}' could not be located"
            )),
            Err(TreeMiss::Ambiguous(paths)) => Err(format!(
                "{repo} has more than one {skill}/SKILL.md ({}), and skills.sh does not say which one it lists",
                paths.join(", ")
            )),
        }
    }
}

/// Why a repo tree listing did not yield exactly one skill location.
#[derive(Debug, PartialEq)]
pub(super) enum TreeMiss {
    /// The listing is complete and has no `<skill>/SKILL.md`.
    Absent,
    /// GitHub truncated the listing and it shows at most one match, so neither
    /// absence nor uniqueness is proven.
    Truncated,
    /// Several directories are named for the skill; picking one would be a guess.
    Ambiguous(Vec<String>),
}

/// The single path of `<skill>/SKILL.md` at any depth in a GitHub recursive
/// tree listing.
pub(super) fn find_skill_md_in_tree(tree: &Value, skill: &str) -> Result<String, TreeMiss> {
    let suffix = format!("/{skill}/SKILL.md");
    let mut matches: Vec<String> = tree
        .get("tree")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("blob"))
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .filter(|path| path.ends_with(&suffix) || *path == &suffix[1..])
        .map(str::to_string)
        .collect();
    // A truncated listing can omit a match: it proves neither that the skill is
    // absent nor that a lone visible match is the only one. Two visible matches
    // are ambiguous either way.
    let truncated = tree.get("truncated").and_then(Value::as_bool) == Some(true);
    if truncated && matches.len() < 2 {
        return Err(TreeMiss::Truncated);
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(TreeMiss::Absent),
        _ => Err(TreeMiss::Ambiguous(matches)),
    }
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
