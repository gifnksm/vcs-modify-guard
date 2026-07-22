use std::fmt::Debug;

use rstest::*;

use super::*;
use crate::testing;

#[fixture]
fn mixed_repository_changes() -> RepositoryChanges {
    let mut files = vec![];
    for i in 0..4 {
        files.push(testing::dirty_file(format!("{i}.dirty.txt")));
    }
    for i in 0..6 {
        files.push(testing::staged_file(format!("{i}.staged.txt")));
    }
    for i in 0..1 {
        files.push(testing::dirty_and_staged_file(format!(
            "{i}.dirty_and_staged.txt"
        )));
    }
    RepositoryChanges::new(files).unwrap()
}

#[track_caller]
fn assert_files_sorted<'a, I>(files: I)
where
    I: Iterator<Item = &'a FileChange>,
{
    let files = files.collect::<Vec<_>>();
    assert!(files.is_sorted_by(|a, b| a.wt_path() < b.wt_path()));
}

#[rstest]
fn repository_changes_files_returns_sorted_unique_files(
    mixed_repository_changes: RepositoryChanges,
) {
    let changes = mixed_repository_changes;

    assert_files_sorted(changes.files());
    assert_files_sorted(changes.dirty_files());
    assert_files_sorted(changes.staged_files());
}

#[track_caller]
fn assert_double_ended_iterator_properties<'a, I, T>(iter: I)
where
    I: DoubleEndedIterator<Item = &'a T> + ExactSizeIterator + Clone,
    T: PartialEq + Debug + ?Sized + 'a,
{
    let forward = iter.clone().collect::<Vec<_>>();
    let mut backward = iter.rev().collect::<Vec<_>>();
    backward.reverse();
    assert_eq!(forward, backward);
}

#[track_caller]
fn assert_exact_size_iterator_properties<'a, I, T>(mut iter: I)
where
    I: ExactSizeIterator<Item = &'a T> + Clone,
    T: PartialEq + Debug + ?Sized + 'a,
{
    let mut remaining_len = iter.len();
    while let Some(_) = iter.next() {
        assert_eq!(iter.len(), remaining_len - 1);
        remaining_len -= 1;
    }
    assert_eq!(remaining_len, 0);
}

#[track_caller]
fn assert_iterator_properties<'a, I, J>(files: I)
where
    I: IntoIterator<Item = &'a FileChange, IntoIter = J> + Clone,
    J: DoubleEndedIterator<Item = &'a FileChange> + ExactSizeIterator + Clone,
{
    let files = files.into_iter();
    assert_double_ended_iterator_properties(files.clone().map(FileChange::wt_path));
    assert_exact_size_iterator_properties(files.clone().map(FileChange::wt_path));
    assert_exact_size_iterator_properties(files.clone().rev().map(FileChange::wt_path));
}

#[rstest]
fn repository_changes_satisfies_iterator_properties(mixed_repository_changes: RepositoryChanges) {
    let changes = mixed_repository_changes;

    assert_iterator_properties(changes.files());
    assert_eq!(changes.files().len(), changes.files.len());

    assert_iterator_properties(changes.dirty_files());
    assert_eq!(changes.dirty_files().len(), changes.num_dirty_files);

    assert_iterator_properties(changes.staged_files());
    assert_eq!(changes.staged_files().len(), changes.num_staged_files);
}
