//! A line diff of two versions of one file, in the shape the rest of seam asks
//! of it: how many of a definition's lines changed, whether it was touched at
//! all, where an unchanged line sits on the other side, and each line in order
//! for a page to draw.
use imara_diff::{sources::byte_lines, Algorithm, Diff, InternedInput};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Side {
    Old,
    New,
}

/// What moved in one file, by 1-based line. `at` maps each old line that went to
/// the new line the change now sits at. The numbers are the ones a `git diff
/// -U0` hunk header gives: where a hunk has no lines on a side, its position
/// there is the line before it, and `delat` and `addat` lean on that.
#[derive(Default)]
pub struct Hunks {
    add: HashSet<u32>,
    del: HashSet<u32>,
    delat: HashSet<u32>,
    addat: HashSet<u32>,
    at: HashMap<u32, u32>,
    blank_new: HashSet<u32>,
    blank_old: HashSet<u32>,
}

fn blocks(old: &[u8], new: &[u8]) -> Vec<(Range<u32>, Range<u32>)> {
    let input = InternedInput::new(byte_lines(old), byte_lines(new));
    let mut diff = Diff::compute(Algorithm::Myers, &input);
    diff.postprocess_lines(&input);
    diff.hunks().map(|h| (h.before, h.after)).collect()
}

fn split_lines(text: &[u8]) -> Vec<&[u8]> {
    let mut lines: Vec<&[u8]> = text.split_inclusive(|b| *b == b'\n').collect();
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

fn is_blank(line: &[u8]) -> bool {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.iter().all(|b| *b == b' ' || *b == b'\t')
}

impl Hunks {
    pub fn between(old: &[u8], new: &[u8]) -> Hunks {
        let (old_lines, new_lines) = (split_lines(old), split_lines(new));
        let mut h = Hunks::default();
        for (before, after) in blocks(old, new) {
            let mut o = before.start + u32::from(!before.is_empty());
            let mut n = after.start + u32::from(!after.is_empty());
            for line in before.clone() {
                h.del.insert(o);
                h.delat.insert(n);
                h.at.insert(o, n);
                if is_blank(old_lines[line as usize]) {
                    h.blank_old.insert(o);
                }
                o += 1;
            }
            for line in after.clone() {
                h.add.insert(n);
                h.addat.insert(o);
                if is_blank(new_lines[line as usize]) {
                    h.blank_new.insert(n);
                }
                n += 1;
            }
        }
        h
    }

    /// How many of the lines in `[s, e]` on this side changed.
    pub fn lines(&self, side: Side, s: u32, e: u32) -> u64 {
        let set = if side == Side::New {
            &self.add
        } else {
            &self.del
        };
        if set.len() < (e.saturating_sub(s) as usize) {
            set.iter().filter(|l| (s..=e).contains(*l)).count() as u64
        } else {
            (s..=e).filter(|l| set.contains(l)).count() as u64
        }
    }

    /// Whether anything in `[s, e]` changed: a line of it, or a change sitting
    /// between two of its lines.
    pub fn touched(&self, side: Side, s: u32, e: u32) -> bool {
        (s..=e).any(|l| match side {
            Side::New => self.add.contains(&l) || (l > s && self.delat.contains(&l)),
            Side::Old => self.del.contains(&l) || (l < e && self.addat.contains(&l)),
        })
    }

    /// The same unchanged line on the other side of the diff.
    pub fn shift(&self, side: Side, l: u32) -> u32 {
        let mut n = i64::from(l);
        for x in 1..=l {
            match side {
                Side::New => {
                    if x < l && self.add.contains(&x) {
                        n -= 1;
                    }
                    if self.delat.contains(&x) {
                        n += 1;
                    }
                }
                Side::Old => {
                    if x < l && self.del.contains(&x) {
                        n -= 1;
                    }
                    if x < l && self.addat.contains(&x) {
                        n += 1;
                    }
                }
            }
        }
        n.max(0) as u32
    }

    pub fn at(&self, old_line: u32) -> Option<u32> {
        self.at.get(&old_line).copied()
    }

    /// The changed lines no definition claims: each line and whether it is blank.
    pub fn added(&self) -> impl Iterator<Item = (u32, bool)> + '_ {
        self.add.iter().map(|l| (*l, self.blank_new.contains(l)))
    }
    pub fn removed(&self) -> impl Iterator<Item = (u32, bool)> + '_ {
        self.del.iter().map(|l| (*l, self.blank_old.contains(l)))
    }
}

/// One line of the whole-file diff, for a page: ` `, `+` or `-`, and its number
/// on each side it has one.
pub struct Line<'a> {
    pub mark: char,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: &'a [u8],
}

/// Every line of the file in diff order. A line of the old side that went comes
/// before the lines that replaced it, the way `git diff` writes a hunk.
pub fn walk<'a>(old: &'a [u8], new: &'a [u8], mut f: impl FnMut(Line<'a>)) {
    let (old_lines, new_lines) = (split_lines(old), split_lines(new));
    let trim = |l: &'a [u8]| l.strip_suffix(b"\n").unwrap_or(l);
    let (mut o, mut n) = (0u32, 0u32);
    for (before, after) in blocks(old, new) {
        while o < before.start {
            f(Line {
                mark: ' ',
                old: Some(o + 1),
                new: Some(n + 1),
                text: trim(old_lines[o as usize]),
            });
            o += 1;
            n += 1;
        }
        for l in before.clone() {
            f(Line {
                mark: '-',
                old: Some(l + 1),
                new: None,
                text: trim(old_lines[l as usize]),
            });
        }
        for l in after.clone() {
            f(Line {
                mark: '+',
                old: None,
                new: Some(l + 1),
                text: trim(new_lines[l as usize]),
            });
        }
        o = before.end;
        n = after.end;
    }
    while (o as usize) < old_lines.len() {
        f(Line {
            mark: ' ',
            old: Some(o + 1),
            new: Some(n + 1),
            text: trim(old_lines[o as usize]),
        });
        o += 1;
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_insertion_is_placed_where_a_hunk_header_places_it() {
        // git writes @@ -1,0 +2,1 @@: the old side's position is the line before.
        let h = Hunks::between(b"a\nb\n", b"a\nx\nb\n");
        assert!(h.add.contains(&2) && h.addat.contains(&1));
        assert!(h.touched(Side::Old, 1, 2) && !h.touched(Side::Old, 2, 2));
    }

    #[test]
    fn a_deletion_is_placed_where_a_hunk_header_places_it() {
        // git writes @@ -2,1 +1,0 @@: the new side's position is the line before.
        let h = Hunks::between(b"a\nx\nb\n", b"a\nb\n");
        assert!(h.del.contains(&2) && h.delat.contains(&1));
        assert_eq!(h.at(2), Some(1));
        assert_eq!(h.shift(Side::Old, 3), 2);
    }

    #[test]
    fn a_walk_gives_every_line_in_diff_order() {
        let mut got = vec![];
        walk(b"a\nb\n", b"a\nc\n", |l| got.push((l.mark, l.old, l.new)));
        assert_eq!(
            got,
            [
                (' ', Some(1), Some(1)),
                ('-', Some(2), None),
                ('+', None, Some(2))
            ]
        );
    }
}
