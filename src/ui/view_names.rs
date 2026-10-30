//! Automatic names for split views, from the directories their panes are in.
//!
//! A view is named after its folder (the last part of the path). Views whose names collide
//! take more of their path until they differ, with the shared middle shown as `…`
//! (`proline/…/testing`, `argo/…/testing`). Of views in exactly the same directory, the oldest
//! keeps the folder's name and the others get a random two-word name that stays fixed.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

/// What a split view is made of, for naming it.
pub struct ViewDirs<'a> {
    /// The view's oldest tab: orders views by age and seeds the random name.
    pub seed: u64,
    /// Directories of its panes (unknown ones left out).
    pub dirs: Vec<&'a Path>,
    /// Directory of the focused pane.
    pub focused: Option<&'a Path>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewName {
    pub name: String,
    /// The directory the name stands for.
    pub dir: PathBuf,
    /// Panes in another directory than `dir`.
    pub elsewhere: usize,
}

/// A name for each view, in the same order (`None` when no pane's directory is known).
pub fn auto_names(views: &[ViewDirs], home: Option<&Path>) -> Vec<Option<ViewName>> {
    let picked: Vec<Option<(PathBuf, usize)>> = views.iter().map(|v| pick_dir(v, home)).collect();
    let parts: Vec<Option<Vec<String>>> = picked
        .iter()
        .map(|p| p.as_ref().map(|(dir, _)| segments(dir)))
        .collect();

    // Oldest first, so it is the one that keeps a duplicated directory's name.
    let mut by_age: Vec<usize> = (0..views.len()).collect();
    by_age.sort_by_key(|&i| views[i].seed);

    let mut names = vec![None; views.len()];
    let mut seen: HashSet<&[String]> = HashSet::new();
    for i in by_age {
        let (Some((dir, elsewhere)), Some(mine)) = (&picked[i], &parts[i]) else {
            continue;
        };
        let name = if home == Some(dir.as_path()) {
            "~".to_string()
        } else if !seen.insert(mine.as_slice()) {
            random_name(views[i].seed)
        } else {
            let others: Vec<&Vec<String>> = parts
                .iter()
                .flatten()
                .filter(|p| *p != mine && p.last() == mine.last())
                .collect();
            let mut k = 1;
            while k < mine.len() && others.iter().any(|o| o.ends_with(&mine[mine.len() - k..])) {
                k += 1;
            }
            let base = mine.last().cloned().unwrap_or_else(|| "/".into());
            match k {
                1 => base,
                2 => format!("{}/{base}", mine[mine.len() - 2]),
                _ => format!("{}/…/{base}", mine[mine.len() - k]),
            }
        };
        names[i] = Some(ViewName {
            name,
            dir: dir.clone(),
            elsewhere: *elsewhere,
        });
    }
    names
}

/// The directory a view stands for: the deepest one its panes share, or the focused pane's
/// when they only share `~` or `/`. Also returns how many panes are elsewhere.
fn pick_dir(view: &ViewDirs, home: Option<&Path>) -> Option<(PathBuf, usize)> {
    let first = *view.dirs.first()?;
    let mut common = first.to_path_buf();
    for d in &view.dirs[1..] {
        while !d.starts_with(&common) {
            if !common.pop() {
                break;
            }
        }
    }
    let shared_nothing = view.dirs.iter().any(|d| *d != first)
        && (home == Some(common.as_path()) || common.parent().is_none());
    if shared_nothing {
        let dir = view.focused.unwrap_or(first).to_path_buf();
        let elsewhere = view.dirs.iter().filter(|d| **d != dir).count();
        return Some((dir, elsewhere));
    }
    Some((common, 0))
}

fn segments(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

const ADJECTIVES: [&str; 16] = [
    "quiet", "amber", "swift", "lunar", "mellow", "brave", "clear", "golden", "subtle", "nomad",
    "misty", "rusty", "sunny", "frosty", "velvet", "hollow",
];
const NOUNS: [&str; 16] = [
    "otter", "heron", "maple", "tide", "compass", "comet", "pine", "finch", "harbor", "wave",
    "jaguar", "dune", "lantern", "meadow", "ember", "falcon",
];

/// Two words picked from `seed`, the same every time.
fn random_name(seed: u64) -> String {
    // splitmix64, so neighbouring seeds land far apart.
    let mut h = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    let adjective = ADJECTIVES[(h % 16) as usize];
    let noun = NOUNS[((h >> 8) % 16) as usize];
    format!("{adjective} {noun}")
}

/// Two letters for the collapsed sidebar: `vertical-tabs-terminal` → `VT`, `argo/…/testing`
/// → `AT`, `rrhh` → `Rr`.
pub fn monogram(name: &str) -> String {
    let initial = |s: &str| s.chars().find(|c| c.is_alphanumeric());
    let parts: Vec<&str> = name.split('/').filter(|s| *s != "…").collect();
    let words: Vec<&str> = if parts.len() > 1 {
        vec![parts[0], parts[parts.len() - 1]]
    } else {
        name.split(['-', '_', ' ', '.'])
            .filter(|s| !s.is_empty())
            .collect()
    };
    let letters: String = match words.as_slice() {
        [a, b, ..] => initial(a)
            .into_iter()
            .chain(initial(b))
            .collect::<String>()
            .to_uppercase(),
        _ => {
            let mut chars = name.chars().filter(|c| c.is_alphanumeric());
            let first = chars.next().map(|c| c.to_uppercase().to_string());
            let second = chars.next().map(|c| c.to_string());
            first.into_iter().chain(second).collect()
        }
    };
    if letters.is_empty() {
        name.chars().take(2).collect()
    } else {
        letters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/u";

    fn view(seed: u64, dirs: &[&'static str]) -> ViewDirs<'static> {
        ViewDirs {
            seed,
            dirs: dirs.iter().map(|d| Path::new(*d)).collect(),
            focused: dirs.first().map(|d| Path::new(*d)),
        }
    }

    fn names(views: &[ViewDirs]) -> Vec<String> {
        auto_names(views, Some(Path::new(HOME)))
            .into_iter()
            .map(|n| n.map(|n| n.name).unwrap_or_default())
            .collect()
    }

    #[test]
    fn names_after_the_shared_folder() {
        let a = "/home/u/code/vertical-tabs-terminal";
        assert_eq!(names(&[view(1, &[a, a])]), ["vertical-tabs-terminal"]);
        // A pane in a subfolder still shares the parent.
        assert_eq!(
            names(&[view(1, &["/home/u/rrhh", "/home/u/rrhh/perfiles"])]),
            ["rrhh"]
        );
        assert_eq!(names(&[view(1, &[HOME, HOME])]), ["~"]);
    }

    #[test]
    fn unrelated_panes_use_the_focused_one() {
        let n = auto_names(
            &[view(1, &["/home/u/a/api", "/home/u/b/web"])],
            Some(Path::new(HOME)),
        );
        let n = n[0].as_ref().unwrap();
        assert_eq!((n.name.as_str(), n.elsewhere), ("api", 1));
        let n = &auto_names(&[view(1, &["/etc", "/var/log"])], None)[0];
        assert_eq!(n.as_ref().unwrap().name, "etc");
    }

    #[test]
    fn collisions_take_more_of_the_path() {
        let v = [
            view(1, &["/home/u/work/proline/envs/testing"]),
            view(2, &["/home/u/work/argo/envs/testing"]),
            view(3, &["/home/u/git/portal/testing"]),
            view(4, &["/home/u/other"]),
        ];
        assert_eq!(
            names(&v),
            [
                "proline/…/testing",
                "argo/…/testing",
                "portal/testing",
                "other"
            ]
        );
        assert_eq!(
            names(&[view(1, &["/a/x/src"]), view(2, &["/b/x/src"])]),
            ["a/…/src", "b/…/src"]
        );
    }

    #[test]
    fn duplicates_get_a_stable_random_name() {
        let a = "/home/u/code/vtt";
        let n = names(&[view(7, &[a]), view(3, &[a])]);
        // The older view (lower seed) keeps the folder's name.
        assert_eq!(n[1], "vtt");
        assert_eq!(n[0], random_name(7));
        assert_eq!(n[0].split(' ').count(), 2);
        assert_eq!(names(&[view(7, &[a]), view(3, &[a])]), n);
    }

    #[test]
    fn unknown_dirs_have_no_name() {
        assert_eq!(auto_names(&[view(1, &[])], None), [None]);
    }

    #[test]
    fn monograms() {
        assert_eq!(monogram("vertical-tabs-terminal"), "VT");
        assert_eq!(monogram("argo/…/testing"), "AT");
        assert_eq!(monogram("envs/testing"), "ET");
        assert_eq!(monogram("rrhh"), "Rr");
        assert_eq!(monogram("quiet otter"), "QO");
        assert_eq!(monogram("~"), "~");
    }
}
