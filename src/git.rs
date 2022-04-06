use git2::{AnnotatedCommit, AutotagOption, FetchOptions, Remote, RemoteCallbacks, Repository};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("merge conflicts detected")]
    MergeConflicts,
    #[error(transparent)]
    Git2(#[from] git2::Error),
}

fn fast_forward(
    repo: &Repository,
    lb: &mut git2::Reference,
    rc: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = match lb.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    println!("{}", msg);
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force(),
    ))?;
    Ok(())
}

pub trait RepositoryExt {
    fn begin_fetch(&self) -> FetchBuilder<'_>;
    fn merge_fetch(
        &self,
        remote_branch: &str,
        fetched_commit: AnnotatedCommit<'_>,
    ) -> Result<bool, GitError>;
    fn simple_merge(
        &self,
        local: &AnnotatedCommit<'_>,
        remote: &AnnotatedCommit<'_>,
    ) -> Result<(), GitError>;
}

impl RepositoryExt for Repository {
    fn begin_fetch(&self) -> FetchBuilder<'_> {
        FetchBuilder::new(self)
    }

    fn merge_fetch(
        &self,
        remote_branch: &str,
        fetched_commit: AnnotatedCommit<'_>,
    ) -> Result<bool, GitError> {
        let analysis = self.merge_analysis(&[&fetched_commit])?;
        if analysis.0.is_fast_forward() {
            Ok(true)
        } else if analysis.0.is_normal() {
            self.simple_merge(
                &self.reference_to_annotated_commit(&self.head()?)?,
                &fetched_commit,
            )
            .map(|_| true)
        } else {
            Ok(false)
        }
    }

    fn simple_merge(
        &self,
        local: &AnnotatedCommit<'_>,
        remote: &AnnotatedCommit<'_>,
    ) -> Result<(), GitError> {
        let local_tree = self.find_commit(local.id())?.tree()?;
        let remote_tree = self.find_commit(remote.id())?.tree()?;
        let ancestor = self
            .find_commit(self.merge_base(local.id(), remote.id())?)?
            .tree()?;
        let mut idx = self.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

        if idx.has_conflicts() {
            self.checkout_index(Some(&mut idx), None)?;
            return Err(GitError::MergeConflicts);
        }
        let result_tree = self.find_tree(idx.write_tree_to(self)?)?;
        // now create the merge commit
        let msg = format!("Merge: {} into {}", remote.id(), local.id());
        let sig = self.signature()?;
        let local_commit = self.find_commit(local.id())?;
        let remote_commit = self.find_commit(remote.id())?;
        // Do our merge commit and set current branch head to that commit.
        let _merge_commit = self.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &msg,
            &result_tree,
            &[&local_commit, &remote_commit],
        )?;
        // Set working tree to match head.
        self.checkout_head(None)?;
        Ok(())
    }
}

pub struct FetchBuilder<'repo> {
    repo: &'repo Repository,
    opts: FetchOptions<'repo>,
    cb: RemoteCallbacks<'repo>,
}

impl<'repo> FetchBuilder<'repo> {
    pub fn new(repo: &'repo Repository) -> Self {
        Self {
            repo,
            opts: FetchOptions::new(),
            cb: RemoteCallbacks::new(),
        }
    }

    pub fn tags(&mut self, opt: AutotagOption) -> &mut Self {
        self.opts.download_tags(opt);
        self
    }

    pub fn cb(&mut self, cb: RemoteCallbacks<'repo>) -> &mut Self {
        self.cb = cb;
        self
    }

    pub fn execute(
        self,
        remote: &'repo mut Remote,
        refs: &[&str],
    ) -> Result<AnnotatedCommit<'repo>, GitError> {
        let repo = self.repo;
        let mut opts = self.opts;
        opts.remote_callbacks(self.cb);
        remote.fetch(refs, Some(&mut opts), None)?;
        repo.reference_to_annotated_commit(&repo.find_reference("FETCH_HEAD")?)
            .map_err(GitError::from)
    }
}
