pub(crate) mod cache;
pub(crate) mod fetcher;
pub(crate) mod spec;

pub(crate) use fetcher::{GitTemplateFetcher, TempCheckout};
pub(crate) use spec::merge_git_template_spec;
