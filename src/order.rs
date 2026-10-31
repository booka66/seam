//! The definitions in the order a reviewer takes them, as threads, and what that
//! order leaves them carrying.
//!
//! A thread reads top down, a definition before what it mentions, and every
//! mention of a changed definition not read yet is a loose end. The order is the
//! one that leaves the fewest loose at any point, as near as a step at a time
//! gets it:
//!
//!   1. Nothing is unmet (read before anything that mentions it) unless it is an
//!      entry point, or a circle leaves no other way in.
//!   2. The fewest loose ends after it.
//!   3. The thread read last goes on: what the latest definition read mentions
//!      comes before what an earlier one does.
//!   4. A mention in a signature before one in a body.
//!   5. The same file as the last one.
//!   6. The table's order, so a change always reads the same way.
//!
//! Definitions no mention joins are read apart, one group through before the
//! next, and each kind of file keeps to its own. Source first; a definition
//! joined to nothing after the rest of its kind. A member goes with its class.
use crate::graph::{Definition, Edge};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Serialize, Clone, Default)]
pub struct Reading {
    pub threads: Vec<Vec<String>>,
    pub unmet: Vec<String>,
    pub loose: i64,
    pub hops: usize,
}

/// A member's id with its member taken off: `a.ts#Box.grow` is `a.ts#Box`.
fn top(id: &str) -> &str {
    let name_at = id.rfind('#').map_or(0, |i| i + 1);
    match id[name_at..].rfind('.') {
        Some(dot) => &id[..name_at + dot],
        None => id,
    }
}

fn rank(kind: &str) -> u8 {
    match kind {
        "source" => 0,
        "tests" => 1,
        "gen" => 2,
        "cfg" => 3,
        _ => 4,
    }
}

struct Web<'g> {
    uses: HashMap<&'g str, Vec<&'g str>>,
    users: HashMap<&'g str, Vec<&'g str>>,
    signature: HashSet<(&'g str, &'g str)>,
    position: HashMap<&'g str, usize>,
    path: HashMap<&'g str, &'g str>,
}

#[derive(Default)]
struct Thread<'g> {
    read: HashMap<&'g str, usize>,
    met: HashMap<&'g str, i64>,
    left: HashMap<&'g str, i64>,
    order: Vec<&'g str>,
    unmet: Vec<&'g str>,
    loose: i64,
    most: i64,
    last: Option<&'g str>,
}

impl<'g> Web<'g> {
    fn users_of(&self, id: &str) -> &[&'g str] {
        self.users.get(id).map_or(&[], Vec::as_slice)
    }
    fn uses_of(&self, id: &str) -> &[&'g str] {
        self.uses.get(id).map_or(&[], Vec::as_slice)
    }

    fn group(&self, start: &'g str) -> Vec<&'g str> {
        let mut seen: HashSet<&str> = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        let mut list = vec![];
        while let Some(x) = queue.pop_front() {
            list.push(x);
            for &y in self.uses_of(x).iter().chain(self.users_of(x)) {
                if seen.insert(y) {
                    queue.push_back(y);
                }
            }
        }
        list.sort_by_key(|id| self.position[id]);
        list
    }

    fn is_unmet(&self, t: &Thread<'g>, x: &str) -> bool {
        !self.users_of(x).is_empty() && t.met.get(x).copied().unwrap_or(0) == 0
    }

    /// The unread definitions reached from `x` by `next`.
    fn reach(
        &self,
        t: &Thread<'g>,
        x: &'g str,
        next: impl Fn(&str) -> Vec<&'g str>,
    ) -> BTreeSet<&'g str> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([x]);
        while let Some(y) = queue.pop_front() {
            for z in next(y) {
                if !t.read.contains_key(z) && seen.insert(z) {
                    queue.push_back(z);
                }
            }
        }
        seen
    }

    /// With every one left unmet, the way in: a circle nothing else leads to,
    /// found by climbing from `x` to whatever mentions it and is not mentioned
    /// back, until nothing is.
    fn way_in(&self, t: &Thread<'g>, mut x: &'g str) -> Vec<&'g str> {
        loop {
            let up = self.reach(t, x, |y| self.users_of(y).to_vec());
            let down = self.reach(t, x, |y| self.uses_of(y).to_vec());
            match up.iter().find(|z| !down.contains(*z)) {
                Some(z) => x = z,
                None => {
                    let mut all: BTreeSet<&str> = up;
                    all.insert(x);
                    return all.into_iter().collect();
                }
            }
        }
    }

    fn thread(&self, ids: &[&'g str]) -> Thread<'g> {
        let mut t = Thread {
            left: ids.iter().map(|&id| (id, self.uses_of(id).len() as i64)).collect(),
            ..Thread::default()
        };
        while t.order.len() < ids.len() {
            let unread: Vec<&str> = ids
                .iter()
                .copied()
                .filter(|id| !t.read.contains_key(id))
                .collect();
            let ready: Vec<&str> = unread
                .iter()
                .copied()
                .filter(|id| !self.is_unmet(&t, id))
                .collect();
            let candidates = if ready.is_empty() {
                self.way_in(&t, unread[0])
            } else {
                ready
            };
            let x = candidates
                .into_iter()
                .min_by_key(|&x| {
                    let readers: Vec<&str> = self
                        .users_of(x)
                        .iter()
                        .copied()
                        .filter(|u| t.read.contains_key(u))
                        .collect();
                    let latest = readers.iter().map(|u| t.read[u] as i64).max().unwrap_or(-1);
                    (
                        i64::from(self.is_unmet(&t, x)),
                        t.left.get(x).copied().unwrap_or(0) - readers.len() as i64,
                        -latest,
                        i64::from(!readers.iter().any(|u| self.signature.contains(&(*u, x)))),
                        i64::from(t.last.is_some_and(|last| self.path[x] != last)),
                        self.position[x] as i64,
                    )
                })
                .expect("a thread with something left has a candidate");
            if self.is_unmet(&t, x) {
                t.unmet.push(x);
            }
            t.loose += t.left.get(x).copied().unwrap_or(0) - t.met.get(x).copied().unwrap_or(0);
            t.read.insert(x, t.order.len());
            t.order.push(x);
            t.last = Some(self.path[x]);
            for &u in self.users_of(x) {
                *t.left.entry(u).or_insert(0) -= 1;
            }
            for &v in self.uses_of(x) {
                *t.met.entry(v).or_insert(0) += 1;
            }
            t.most = t.most.max(t.loose);
        }
        t
    }
}

pub fn reading(definitions: &[Definition], edges: &[Edge]) -> Reading {
    let tops: Vec<&Definition> = definitions.iter().filter(|d| d.class.is_none()).collect();
    let by: HashMap<&str, &Definition> = tops.iter().map(|d| (d.id.as_str(), *d)).collect();
    let mut pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut signature = HashSet::new();
    for e in edges {
        let (from, to) = (top(&e.from), top(&e.to));
        let (Some(f), Some(t)) = (by.get(from), by.get(to)) else {
            continue;
        };
        if from == to || f.container.kind != t.container.kind {
            continue;
        }
        pairs.insert((from, to));
        if e.facet == "iface" {
            signature.insert((from, to));
        }
    }
    let mut uses: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut users: HashMap<&str, Vec<&str>> = HashMap::new();
    for &(from, to) in &pairs {
        uses.entry(from).or_default().push(to);
        users.entry(to).or_default().push(from);
    }
    let web = Web {
        uses,
        users,
        signature,
        position: tops
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id.as_str(), i))
            .collect(),
        path: tops
            .iter()
            .map(|d| (d.id.as_str(), d.path.as_str()))
            .collect(),
    };
    let mut done: HashSet<&str> = HashSet::new();
    let mut groups: Vec<Vec<&str>> = vec![];
    for d in &tops {
        if !done.contains(d.id.as_str()) {
            let g = web.group(&d.id);
            done.extend(g.iter().copied());
            groups.push(g);
        }
    }
    groups.sort_by_key(|g| (rank(&by[g[0]].container.kind), g.len() == 1));
    let threads: Vec<Thread> = groups.iter().map(|g| web.thread(g)).collect();
    let order: Vec<&str> = threads
        .iter()
        .flat_map(|t| t.order.iter().copied())
        .collect();
    Reading {
        threads: threads
            .iter()
            .map(|t| t.order.iter().map(|s| s.to_string()).collect())
            .collect(),
        unmet: threads
            .iter()
            .flat_map(|t| t.unmet.iter().map(|s| s.to_string()))
            .collect(),
        loose: threads.iter().map(|t| t.most).max().unwrap_or(0),
        hops: order
            .windows(2)
            .filter(|w| by[w[0]].path != by[w[1]].path)
            .count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Container;

    /// Each word is `from>to`, a mention of to in from, with `:i` when it is in
    /// the signature. The result reads: the order / how many loose / what was unmet.
    fn order(words: &[&str]) -> String {
        let mut names = BTreeSet::new();
        let edges: Vec<Edge> = words
            .iter()
            .map(|w| {
                let (pair, facet) = w.strip_suffix(":i").map_or((*w, "body"), |p| (p, "iface"));
                let (from, to) = pair.split_once('>').unwrap();
                names.insert(from);
                names.insert(to);
                Edge {
                    from: format!("a.ts#{from}"),
                    to: format!("a.ts#{to}"),
                    facet: facet.into(),
                    new: false,
                }
            })
            .collect();
        let definitions: Vec<Definition> = names
            .into_iter()
            .map(|n| Definition {
                id: format!("a.ts#{n}"),
                path: "a.ts".into(),
                kind: String::new(),
                name: n.into(),
                class: None,
                exported: None,
                change: String::new(),
                side: String::new(),
                start: None,
                end: None,
                open: None,
                added: None,
                removed: None,
                head: String::new(),
                base: None,
                staged: None,
                unstaged: None,
                root: false,
                container: Container {
                    file: "a.ts".into(),
                    kind: "source".into(),
                },
            })
            .collect();
        let r = reading(&definitions, &edges);
        let short = |s: &String| s.trim_start_matches("a.ts#").to_string();
        let thread: Vec<String> = r.threads.iter().flatten().map(short).collect();
        let unmet: Vec<String> = r.unmet.iter().map(short).collect();
        format!("{} / {} / {}", thread.join(" "), r.loose, unmet.join(" "))
    }

    #[test]
    fn ends_before_threads() {
        assert_eq!(order(&["A>B", "A>C", "A>D", "B>E"]), "A C D B E / 3 / ");
    }
    #[test]
    fn a_signature_comes_first() {
        assert_eq!(order(&["A>B", "A>C:i"]), "A C B / 2 / ");
    }
    #[test]
    fn the_last_thread_goes_on() {
        assert_eq!(
            order(&["A>B", "A>C", "B>D", "C>F", "D>E"]),
            "A B D E C F / 2 / "
        );
    }
    #[test]
    fn a_circle_is_cut() {
        assert_eq!(order(&["B>C", "C>B", "C>D"]), "B C D / 1 / B");
    }
    #[test]
    fn the_circle_in_front_is_the_way_in() {
        assert_eq!(
            order(&["X>Y", "Y>X", "Y>B", "B>C", "C>B"]),
            "X Y B C / 1 / X"
        );
    }
}
